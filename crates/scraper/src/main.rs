//! Runs the scheduler and dumps to the specified sink.
use std::fs;
use std::io::Write;
use std::{io::stdout, path::PathBuf, thread::available_parallelism};

use tokio::sync::mpsc;

use anyhow::{Context as _, bail};
use clap::Parser;
use clap_verbosity_flag::InfoLevel;
use url::Url;

use wiki_scraper::{APP_USER_AGENT, crawl};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Optional starting seed of URLs to scrape.
    seed: Option<Vec<Url>>,

    /// Number of workers to spawn.
    ///
    /// Defaults to [`available_parallelism`].
    #[arg(long)]
    workers: Option<usize>,

    /// Verbosity setting.
    ///
    /// Defaults to info level.
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity<InfoLevel>,

    /// Output file to save to.
    #[arg(long, short)]
    output: Option<PathBuf>,
}

/// Main entry point for the scraper.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::SystemTime)
        .with_level(true)
        .with_max_level(args.verbosity)
        .init();

    let mut sink: Box<dyn Write> = match &args.output {
        Some(path) => Box::new(fs::File::create(path)?),
        None => Box::new(stdout().lock()),
    };
    let seed = args.seed.unwrap_or(vec![Url::parse(
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
    )?]);
    let host = match seed.first() {
        Some(url) => url.host_str(),
        None => bail!("need to provide a starting seed"),
    };
    if !seed.iter().all(|s| s.host_str() == host) {
        bail!("not all seeds have the same host")
    }

    let num_workers = args.workers.unwrap_or(
        available_parallelism()
            .with_context(|| "unable to get available_parallelism")?
            .into(),
    );

    let (tx, mut rx) = mpsc::channel(32);
    let handle = tokio::spawn(crawl(APP_USER_AGENT, num_workers, seed, tx));
    while let Some(page) = rx.recv().await {
        serde_json::to_writer(&mut sink, &page)?;
        writeln!(sink)?;
    }
    handle.await??;

    Ok(())
}
