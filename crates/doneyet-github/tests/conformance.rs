use std::time::Duration;

use doneyet_contract::suite::{CONTRACT_TOKEN, ProviderFactory, run_suite};
use doneyet_core::model::RepoRef;
use doneyet_core::ports::PipelineProvider;
use doneyet_github::{GithubConfig, GithubProvider};

struct GithubFactory {
    repo: RepoRef,
    token: String,
    retry: doneyet_github::RetryPolicy,
}

impl GithubFactory {
    fn new() -> Self {
        Self {
            repo: RepoRef {
                owner: "acme".to_string(),
                name: "api".to_string(),
            },
            token: CONTRACT_TOKEN.to_string(),
            retry: doneyet_github::RetryPolicy {
                max_retries: 2,
                base_delay: Duration::from_millis(1),
            },
        }
    }
}

impl ProviderFactory for GithubFactory {
    async fn make(&self, base_url: String) -> Box<dyn PipelineProvider> {
        let config = GithubConfig {
            api_base: base_url,
            token: Some(self.token.clone()),
            retry: self.retry,
        };
        Box::new(GithubProvider::new(self.repo.clone(), config).expect("provider builds"))
    }
}

#[tokio::test]
async fn github_provider_passes_contract_suite() {
    run_suite(&GithubFactory::new()).await;
}
