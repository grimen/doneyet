use std::sync::Mutex;
use std::time::Duration;

use doneyet_core::model::{Annotation, Job, RepoRef, RunsPage, RunsQuery, WorkflowRun};
use doneyet_core::ports::{AnnotationSource, LogSource, ProviderError, RunSource};
use reqwest::header::{ACCEPT, AUTHORIZATION, ETAG, HeaderMap, HeaderValue, IF_NONE_MATCH};
use reqwest::{Client, StatusCode, Url};

use crate::dto::{RawAnnotation, RawJob, RawJobsPage, RawPull, RawRun, RawRunsPage};
use crate::etag::EtagCache;
use crate::retry::RetryPolicy;
use crate::status::{error_message, is_rate_limited, map_forbidden, retry_after_hint};

pub const DEFAULT_API_BASE: &str = "https://api.github.com";
const MAX_PAGES: usize = 20;

#[derive(Debug, Clone)]
pub struct GithubConfig {
    pub api_base: String,
    pub token: Option<String>,
    pub retry: RetryPolicy,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            api_base: DEFAULT_API_BASE.to_string(),
            token: None,
            retry: RetryPolicy::default(),
        }
    }
}

pub struct GithubProvider {
    base: String,
    repo: RepoRef,
    http: Client,
    retry: RetryPolicy,
    etags: Mutex<EtagCache>,
}

struct Fetched {
    body: String,
    link: Option<String>,
}

impl GithubProvider {
    pub fn new(repo: RepoRef, config: GithubConfig) -> Result<Self, ProviderError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "x-github-api-version",
            HeaderValue::from_static("2022-11-28"),
        );
        if let Some(token) = &config.token {
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| ProviderError::Auth)?;
            headers.insert(AUTHORIZATION, value);
        }
        let http = Client::builder()
            .default_headers(headers)
            .user_agent(concat!("doneyet/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        Ok(Self {
            base: config.api_base.trim_end_matches('/').to_string(),
            repo,
            http,
            retry: config.retry,
            etags: Mutex::new(EtagCache::default()),
        })
    }

    fn runs_url(&self) -> Result<Url, ProviderError> {
        Url::parse(&format!("{}/repos/{}/actions/runs", self.base, self.repo))
            .map_err(|e| ProviderError::Parse(e.to_string()))
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<T, ProviderError> {
        let fetched = self.fetch(url).await?;
        serde_json::from_str(&fetched.body).map_err(|e| ProviderError::Parse(format!("{url}: {e}")))
    }

    async fn fetch(&self, url: &str) -> Result<Fetched, ProviderError> {
        let cached = self
            .etags
            .lock()
            .expect("etag cache poisoned")
            .get(url)
            .cloned();
        let mut attempt = 0u32;
        loop {
            let mut request = self.http.get(url);
            if let Some((etag, _)) = &cached {
                request = request.header(IF_NONE_MATCH, etag.as_str());
            }
            let response = match request.send().await {
                Ok(response) => response,
                Err(error) => {
                    let Some(delay) = self.retry_delay(&mut attempt) else {
                        return Err(ProviderError::Transport(error.to_string()));
                    };
                    tracing::warn!(url, attempt, error = %error, "transport error; retrying github request");
                    tokio::time::sleep(delay).await;
                    continue;
                }
            };
            let status = response.status();
            if status.is_success() {
                let headers = response.headers().clone();
                let etag = headers
                    .get(ETAG)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let link = headers
                    .get("link")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let body = match response.text().await {
                    Ok(body) => body,
                    Err(error) => {
                        let Some(delay) = self.retry_delay(&mut attempt) else {
                            return Err(ProviderError::Transport(error.to_string()));
                        };
                        tracing::warn!(url, attempt, error = %error, "transport error; retrying github request");
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                };
                if let Some(etag) = etag {
                    self.etags
                        .lock()
                        .expect("etag cache poisoned")
                        .store(url, etag, body.clone());
                }
                return Ok(Fetched { body, link });
            }
            match status {
                StatusCode::NOT_MODIFIED => {
                    tracing::debug!(url, "github returned 304, reusing cached body");
                    let body = cached.map(|(_, body)| body).ok_or_else(|| {
                        ProviderError::Parse(format!("{url}: 304 without a cached body"))
                    })?;
                    return Ok(Fetched { body, link: None });
                }
                StatusCode::UNAUTHORIZED => return Err(ProviderError::Auth),
                StatusCode::NOT_FOUND => {
                    return Err(ProviderError::NotFound(error_message(response).await));
                }
                StatusCode::TOO_MANY_REQUESTS | StatusCode::FORBIDDEN => {
                    let headers = response.headers().clone();
                    let remaining = header_str(&headers, "x-ratelimit-remaining");
                    let retry_after = header_str(&headers, "retry-after");
                    let reset = header_str(&headers, "x-ratelimit-reset");
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        return Err(ProviderError::RateLimited {
                            retry_after: retry_after_hint(retry_after, reset, now_secs()),
                        });
                    }
                    let message = error_message(response).await;
                    return Err(map_forbidden(
                        is_rate_limited(remaining),
                        retry_after,
                        reset,
                        now_secs(),
                        message,
                    ));
                }
                s if s.is_server_error() => {
                    let Some(delay) = self.retry_delay(&mut attempt) else {
                        return Err(ProviderError::Other(format!(
                            "{url}: upstream returned {s}"
                        )));
                    };
                    tracing::warn!(url, status = %s, attempt, "retrying github request");
                    tokio::time::sleep(delay).await;
                }
                s => {
                    return Err(ProviderError::Other(format!(
                        "{url}: unexpected status {s}"
                    )));
                }
            }
        }
    }

    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, ProviderError> {
        let mut attempt = 0u32;
        loop {
            let response = match self.http.get(url).send().await {
                Ok(response) => response,
                Err(error) => {
                    let Some(delay) = self.retry_delay(&mut attempt) else {
                        return Err(ProviderError::Transport(error.to_string()));
                    };
                    tracing::warn!(url, attempt, error = %error, "transport error; retrying github request");
                    tokio::time::sleep(delay).await;
                    continue;
                }
            };
            let status = response.status();
            if status.is_success() {
                let bytes = match response.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(error) => {
                        let Some(delay) = self.retry_delay(&mut attempt) else {
                            return Err(ProviderError::Transport(error.to_string()));
                        };
                        tracing::warn!(url, attempt, error = %error, "transport error; retrying github request");
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                };
                return Ok(bytes);
            }
            match status {
                StatusCode::UNAUTHORIZED => return Err(ProviderError::Auth),
                StatusCode::NOT_FOUND => {
                    return Err(ProviderError::NotFound(error_message(response).await));
                }
                StatusCode::TOO_MANY_REQUESTS | StatusCode::FORBIDDEN => {
                    let headers = response.headers().clone();
                    let remaining = header_str(&headers, "x-ratelimit-remaining");
                    let retry_after = header_str(&headers, "retry-after");
                    let reset = header_str(&headers, "x-ratelimit-reset");
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        return Err(ProviderError::RateLimited {
                            retry_after: retry_after_hint(retry_after, reset, now_secs()),
                        });
                    }
                    let message = error_message(response).await;
                    return Err(map_forbidden(
                        is_rate_limited(remaining),
                        retry_after,
                        reset,
                        now_secs(),
                        message,
                    ));
                }
                s if s.is_server_error() => {
                    let Some(delay) = self.retry_delay(&mut attempt) else {
                        return Err(ProviderError::Other(format!(
                            "{url}: upstream returned {s}"
                        )));
                    };
                    tracing::warn!(url, status = %s, attempt, "retrying github request");
                    tokio::time::sleep(delay).await;
                }
                s => {
                    return Err(ProviderError::Other(format!(
                        "{url}: unexpected status {s}"
                    )));
                }
            }
        }
    }

    fn retry_delay(&self, attempt: &mut u32) -> Option<Duration> {
        if !self.retry.should_retry(*attempt) {
            return None;
        }
        *attempt += 1;
        Some(self.retry.delay_for(*attempt))
    }
}

#[async_trait::async_trait]
impl RunSource for GithubProvider {
    async fn list_runs(&self, query: &RunsQuery) -> Result<RunsPage, ProviderError> {
        let per_page = query.limit.clamp(1, 100);
        let mut url = self.runs_url()?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("per_page", &per_page.to_string());
            if let Some(branch) = &query.branch {
                pairs.append_pair("branch", branch);
            }
            if let Some(event) = &query.event {
                pairs.append_pair("event", event);
            }
            if let Some(head_sha) = &query.head_sha {
                pairs.append_pair("head_sha", head_sha);
            }
        }
        let mut total_count = 0u64;
        let mut runs = Vec::new();
        let mut next = Some(url.to_string());
        for _ in 0..MAX_PAGES {
            let Some(current) = next else { break };
            let fetched = self.fetch(&current).await?;
            let page: RawRunsPage = serde_json::from_str(&fetched.body)
                .map_err(|e| ProviderError::Parse(format!("{current}: {e}")))?;
            if total_count == 0 {
                total_count = page.total_count;
            }
            runs.extend(page.workflow_runs.into_iter().map(RawRun::into_model));
            if runs.len() >= query.limit as usize {
                break;
            }
            next = link_next(fetched.link.as_deref());
        }
        runs.truncate(query.limit as usize);
        Ok(RunsPage { total_count, runs })
    }

    async fn get_run(&self, run_id: u64) -> Result<WorkflowRun, ProviderError> {
        let url = format!("{}/repos/{}/actions/runs/{run_id}", self.base, self.repo);
        Ok(self.get_json::<RawRun>(&url).await?.into_model())
    }

    async fn list_jobs(&self, run_id: u64) -> Result<Vec<Job>, ProviderError> {
        let first = format!(
            "{}/repos/{}/actions/runs/{run_id}/jobs?per_page=100",
            self.base, self.repo
        );
        let mut jobs = Vec::new();
        let mut next = Some(first);
        for _ in 0..MAX_PAGES {
            let Some(current) = next else { break };
            let fetched = self.fetch(&current).await?;
            let page: RawJobsPage = serde_json::from_str(&fetched.body)
                .map_err(|e| ProviderError::Parse(format!("{current}: {e}")))?;
            jobs.extend(page.jobs.into_iter().map(RawJob::into_model));
            next = link_next(fetched.link.as_deref());
        }
        Ok(jobs)
    }
    async fn pr_head_sha(&self, number: u64) -> Result<String, ProviderError> {
        let url = format!("{}/repos/{}/pulls/{number}", self.base, self.repo);
        let pull: RawPull = self.get_json(&url).await?;
        Ok(pull.head.sha)
    }
}

#[async_trait::async_trait]
impl AnnotationSource for GithubProvider {
    async fn list_annotations(&self, job_id: u64) -> Result<Vec<Annotation>, ProviderError> {
        let url = format!(
            "{}/repos/{}/check-runs/{job_id}/annotations?per_page=100",
            self.base, self.repo
        );
        let raw: Vec<RawAnnotation> = self.get_json(&url).await?;
        Ok(raw.into_iter().map(|a| a.into_model(job_id)).collect())
    }
}

#[async_trait::async_trait]
impl LogSource for GithubProvider {
    async fn job_logs(&self, job_id: u64) -> Result<Vec<u8>, ProviderError> {
        let url = format!(
            "{}/repos/{}/actions/jobs/{job_id}/logs",
            self.base, self.repo
        );
        self.fetch_bytes(&url).await
    }
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn link_next(header: Option<&str>) -> Option<String> {
    let header = header?;
    for segment in header.split(',') {
        let segment = segment.trim();
        if segment.contains("rel=\"next\"") {
            let start = segment.find('<')? + 1;
            let end = segment.find('>')?;
            if end > start {
                return Some(segment[start..end].to_string());
            }
        }
    }
    None
}
