pub mod keys;
pub mod repo;

use clap::{Args, Parser, Subcommand};
use doneyet_app::{WatchConfig, WatchEngine, WatchTarget};
use doneyet_core::model::{Outcome, Phase, RepoRef, RunsQuery, World};
use doneyet_core::ports::{AnnotationSource, PushSource, Renderer, RunSource};
use doneyet_github::{GithubConfig, GithubProvider};
use doneyet_ux::{TermRenderer, stdout_color};
use std::process::ExitCode;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Parser)]
#[command(
    name = "doneyet",
    version,
    about = "doneyet — exits when it's done yet"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Watch the latest matching run live
    Watch {
        #[arg(help = "OWNER/NAME; defaults to the origin remote of the current directory")]
        repo: Option<String>,
        #[arg(short, long)]
        branch: Option<String>,
        #[arg(long)]
        commit: Option<String>,
        #[arg(long, default_value_t = 3)]
        interval: u64,
        #[arg(
            long,
            requires = "webhook_secret",
            help = "Listen for GitHub webhook pushes on ADDR (e.g. 127.0.0.1:4567)"
        )]
        webhook: Option<String>,
        #[arg(
            long,
            requires = "webhook",
            help = "Secret used to validate X-Hub-Signature-256"
        )]
        webhook_secret: Option<String>,
        #[arg(long, help = "Record every rendered frame to PATH as JSONL")]
        record: Option<String>,
        #[command(flatten)]
        common: CommonArgs,
    },
    /// List recent runs
    Runs {
        #[arg(help = "OWNER/NAME; defaults to the origin remote of the current directory")]
        repo: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: u32,
        #[arg(short, long)]
        branch: Option<String>,
        #[arg(long)]
        event: Option<String>,
        #[command(flatten)]
        common: CommonArgs,
    },
    /// Inspect a single run
    Run {
        run_id: u64,
        #[arg(
            long,
            help = "OWNER/NAME; defaults to the origin remote of the current directory"
        )]
        repo: Option<String>,
        #[arg(
            long,
            num_args = 0..=1,
            default_missing_value = "20",
            help = "Print the last N log lines of failed jobs (default 20)"
        )]
        logs_failed: Option<usize>,
        #[command(flatten)]
        common: CommonArgs,
    },
    /// Replay a recorded session (--record) offline
    Replay {
        path: String,
        #[arg(long, help = "Replay at the recorded wall-clock pace")]
        realtime: bool,
        #[command(flatten)]
        common: CommonArgs,
    },
}

#[derive(Debug, Args)]
pub struct CommonArgs {
    #[arg(long, default_value = doneyet_github::DEFAULT_API_BASE)]
    pub api_base: String,
    #[arg(long, env = "DONEYET_TOKEN")]
    pub token: Option<String>,
    #[arg(long)]
    pub no_color: bool,
    #[arg(long)]
    pub width: Option<usize>,
    #[arg(
        long,
        default_value = "default",
        help = "Render theme: built-in name (default, ascii) or path to a JSON theme file"
    )]
    pub theme: String,
}

fn resolve_theme(common: &CommonArgs) -> anyhow::Result<doneyet_ux::Theme> {
    doneyet_ux::load_theme(&common.theme).map_err(|error| anyhow::anyhow!("{error}"))
}

const KNOWN_SUBCOMMANDS: [&str; 8] = [
    "watch",
    "runs",
    "run",
    "help",
    "--help",
    "-h",
    "--version",
    "-V",
];

pub fn inject_default_subcommand(args: Vec<String>) -> Vec<String> {
    if let Some(first) = args.get(1) {
        if !first.starts_with('-')
            && !KNOWN_SUBCOMMANDS.contains(&first.as_str())
            && first.contains('/')
        {
            let mut out = args;
            out.insert(1, "watch".to_string());
            return out;
        }
    }
    args
}

pub fn main() -> ExitCode {
    let cli = Cli::parse_from(inject_default_subcommand(std::env::args().collect()));
    let runtime = tokio::runtime::Runtime::new().expect("spawn tokio runtime");
    match runtime.block_on(dispatch(cli)) {
        Ok(code) => ExitCode::from((code as i32).clamp(0, 255) as u8),
        Err(error) => {
            eprintln!("doneyet: {error}");
            ExitCode::from(4)
        }
    }
}

async fn dispatch(cli: Cli) -> anyhow::Result<u32> {
    match cli.command {
        Command::Watch {
            repo,
            branch,
            commit,
            interval,
            webhook,
            webhook_secret,
            record,
            common,
        } => {
            watch(
                WatchOptions {
                    repo,
                    branch,
                    commit,
                    interval,
                    webhook,
                    webhook_secret,
                    record,
                },
                common,
            )
            .await
        }
        Command::Runs {
            repo,
            limit,
            branch,
            event,
            common,
        } => list_runs(repo, limit, branch, event, common).await,
        Command::Run {
            run_id,
            repo,
            logs_failed,
            common,
        } => inspect_run(run_id, repo, logs_failed, common).await,
        Command::Replay {
            path,
            realtime,
            common,
        } => replay_file(path, realtime, common).await,
    }
}

async fn replay_file(path: String, realtime: bool, common: CommonArgs) -> anyhow::Result<u32> {
    let events = doneyet_ux::read_records(std::path::Path::new(&path))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if events.is_empty() {
        anyhow::bail!("no records found in {path:?}");
    }
    let mut renderer = TermRenderer::with_theme(
        Box::new(std::io::stdout()),
        resolve_theme(&common)?,
        color_enabled(&common),
        terminal_width(&common),
        Box::new(jiff::Timestamp::now),
    );
    let deltas = doneyet_ux::frame_deltas(&events);
    for (index, event) in events.iter().enumerate() {
        if realtime && index > 0 {
            tokio::time::sleep(deltas[index - 1]).await;
        }
        match event {
            doneyet_ux::ReplayEvent::Render {
                world,
                events: frame_events,
                ..
            } => renderer.render(world, frame_events)?,
            doneyet_ux::ReplayEvent::Finish { outcome, .. } => renderer.finish(outcome)?,
        }
    }
    match events.last() {
        Some(doneyet_ux::ReplayEvent::Finish { outcome, .. }) => {
            Ok(outcome.conclusion.exit_code() as u32)
        }
        _ => {
            eprintln!("doneyet: recording has no finish record; exiting 0");
            Ok(0)
        }
    }
}

fn build_provider(repo: RepoRef, common: &CommonArgs) -> anyhow::Result<GithubProvider> {
    let token = common
        .token
        .clone()
        .or_else(doneyet_github::token::resolve_from_environment);
    let config = GithubConfig {
        api_base: common.api_base.clone(),
        token,
        ..GithubConfig::default()
    };
    GithubProvider::new(repo, config).map_err(Into::into)
}

fn color_enabled(common: &CommonArgs) -> bool {
    !common.no_color && stdout_color()
}

fn terminal_width(common: &CommonArgs) -> usize {
    common
        .width
        .unwrap_or_else(|| {
            crossterm::terminal::size()
                .map(|(cols, _)| cols as usize)
                .unwrap_or(100)
        })
        .max(40)
}

struct WatchOptions {
    repo: Option<String>,
    branch: Option<String>,
    commit: Option<String>,
    interval: u64,
    webhook: Option<String>,
    webhook_secret: Option<String>,
    record: Option<String>,
}

async fn watch(opts: WatchOptions, common: CommonArgs) -> anyhow::Result<u32> {
    let repo = repo::resolve(opts.repo.as_deref())?;
    let query = RunsQuery {
        repo: repo.clone(),
        branch: opts.branch,
        head_sha: opts.commit,
        event: None,
        limit: 1,
    };
    let provider = build_provider(repo.clone(), &common)?;
    let (hint_tx, channel_push) = doneyet_app::ChannelPushSource::channel();
    let push: Box<dyn doneyet_core::ports::PushSource> = match (opts.webhook, opts.webhook_secret) {
        (Some(addr), Some(secret)) => {
            let mut webhook_push =
                doneyet_github::webhook::WebhookPush::start(&addr, secret).await?;
            eprintln!(
                "doneyet: webhook listening on http://{}/",
                webhook_push.local_addr()
            );
            let tx = hint_tx.clone();
            tokio::spawn(async move {
                while let Ok(hint) = webhook_push.wait().await {
                    if tx.send(hint).is_err() {
                        break;
                    }
                }
            });
            Box::new(channel_push)
        }
        _ => Box::new(channel_push),
    };
    let shutdown = CancellationToken::new();
    let cancel = shutdown.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            cancel.cancel();
        }
    });
    let raw_mode = keys::RawModeGuard::enable();
    if raw_mode.is_some() {
        eprintln!("(q quit · r refresh)");
        keys::spawn(repo.clone(), shutdown.clone(), hint_tx.clone());
    }
    let stdout: Box<dyn std::io::Write + Send> = if raw_mode.is_some() {
        Box::new(doneyet_ux::writer::CrLfWriter::new(std::io::stdout()))
    } else {
        Box::new(std::io::stdout())
    };
    let term = TermRenderer::with_theme(
        stdout,
        resolve_theme(&common)?,
        color_enabled(&common),
        terminal_width(&common),
        Box::new(jiff::Timestamp::now),
    );
    let renderer: Box<dyn doneyet_core::ports::Renderer> = match opts.record {
        Some(path) => Box::new(doneyet_ux::record::TeeRenderer::new(
            Box::new(term),
            std::path::Path::new(&path),
        )?),
        None => Box::new(term),
    };
    let config = WatchConfig {
        active_interval: Duration::from_secs(opts.interval.max(1)),
        idle_interval: Duration::from_secs(20),
    };
    let mut engine = WatchEngine::new(repo, Box::new(provider), renderer, push, config, shutdown);
    let outcome = engine.watch(WatchTarget::Latest(query)).await?;
    drop(raw_mode);
    Ok(outcome.exit_code() as u32)
}

async fn list_runs(
    repo_arg: Option<String>,
    limit: u32,
    branch: Option<String>,
    event: Option<String>,
    common: CommonArgs,
) -> anyhow::Result<u32> {
    let repo = repo::resolve(repo_arg.as_deref())?;
    let provider = build_provider(repo.clone(), &common)?;
    let query = RunsQuery {
        repo,
        branch,
        head_sha: None,
        event,
        limit,
    };
    let page = provider.list_runs(&query).await?;
    println!(
        "{}",
        doneyet_ux::runs_table_with(
            &page,
            &resolve_theme(&common)?,
            color_enabled(&common),
            terminal_width(&common),
            jiff::Timestamp::now()
        )
    );
    Ok(0)
}

async fn inspect_run(
    run_id: u64,
    repo_arg: Option<String>,
    logs_failed: Option<usize>,
    common: CommonArgs,
) -> anyhow::Result<u32> {
    let repo = repo::resolve(repo_arg.as_deref())?;
    let provider = build_provider(repo.clone(), &common)?;
    let run = provider.get_run(run_id).await?;
    let jobs = provider.list_jobs(run_id).await?;
    let failed_ids: Vec<u64> = jobs
        .iter()
        .filter(|job| job.phase.is_failed())
        .map(|job| job.id)
        .collect();
    let mut annotations = Vec::new();
    for job_id in &failed_ids {
        if let Ok(mut found) = provider.list_annotations(*job_id).await {
            annotations.append(&mut found);
        }
    }
    let world = World {
        repo,
        run,
        jobs,
        annotations,
        stats: None,
    };
    let color = color_enabled(&common);
    let width = terminal_width(&common);
    println!(
        "{}",
        TermRenderer::frame_text(&world, color, width, jiff::Timestamp::now())
    );
    if let Some(tail) = logs_failed {
        print_failed_logs(&provider, &world.jobs, tail).await;
    }
    if let Phase::Done(conclusion) = &world.run.phase {
        println!();
        println!(
            "{}",
            TermRenderer::finish_text(
                &Outcome {
                    run: world.run.ref_of(),
                    conclusion: conclusion.clone()
                },
                color
            )
        );
        return Ok(conclusion.exit_code() as u32);
    }
    Ok(0)
}

async fn print_failed_logs(
    provider: &doneyet_github::GithubProvider,
    jobs: &[doneyet_core::model::Job],
    tail: usize,
) {
    use doneyet_core::ports::LogSource;
    for job in jobs.iter().filter(|job| job.phase.is_failed()) {
        println!();
        println!("── logs: {} ──", job.name);
        match provider.job_logs(job.id).await {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                let lines: Vec<&str> = text.lines().collect();
                let start = lines.len().saturating_sub(tail);
                for line in &lines[start..] {
                    println!("{line}");
                }
            }
            Err(error) => println!("  (logs unavailable: {error})"),
        }
    }
}
