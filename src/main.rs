use std::{collections::VecDeque, thread::available_parallelism};

use anyhow::{Context, anyhow, bail};
use clap::Parser;
use clap_verbosity_flag::InfoLevel;
use reqwest::ClientBuilder;
use rustc_hash::FxHashSet;
use scraper::Selector;
use serde::Serialize;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use url::Url;

/// Crawler errors.
#[derive(thiserror::Error, Debug)]
enum CrawlError {
    #[error("fetch failed: {0}")]
    Fetch(#[from] reqwest::Error),
    #[error("parse failed: {0}")]
    Parse(String),
}

/// See:
/// <https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy>.
static APP_USER_AGENT: &str = concat!(
    "webviz-bot",
    "/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/micahkepe)",
    "; ",
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);

#[derive(Debug)]
struct RawPage {
    url: Url,
    content: String,
}

#[derive(Debug, Serialize)]
struct ParsedPage {
    url: Url,
    /// NOTE: outbound here meaning a different path still within the same
    /// origin, e.g., `https://foo.example.com/bar` ->
    /// `https://foo.example.com/baz`, but **not** `https://example.com/foo` ->
    /// `https://buzz.com/bar` **nor** `https://foo.example.com/bar`
    /// `https://bar.example.com/bar`.
    outbound_links: Vec<Url>,
}

#[derive(Debug, Default)]
struct Frontier {
    queued: VecDeque<Url>,
    visited: FxHashSet<Url>,
}

impl Frontier {
    fn with_seed<T>(seed: T) -> Self
    where
        T: IntoIterator<Item = Url>,
    {
        Self {
            queued: seed.into_iter().collect(),
            visited: FxHashSet::default(),
        }
    }

    fn try_enqueue(&mut self, url: Url) -> bool {
        if self.visited.insert(url.clone()) {
            self.queued.push_back(url);
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
struct Scheduler {
    /// The [crawl frontier].
    ///
    /// [crawl frontier]: https://en.wikipedia.org/wiki/Crawl_frontier
    frontier: Frontier,
    /// Crawl job transceiver for sending URLs to fetch tasks.
    job_tx: async_channel::Sender<Url>,
    /// Parsed page results receiver from the page parser tasks.
    results_rx: mpsc::Receiver<Result<ParsedPage, CrawlError>>,
    /// Track in-flight tasks.
    in_flight: usize,
}

impl Scheduler {
    pub(crate) fn new<T>(
        seed: T,
        job_tx: async_channel::Sender<Url>,
        results_rx: mpsc::Receiver<Result<ParsedPage, CrawlError>>,
    ) -> Self
    where
        T: IntoIterator<Item = Url>,
    {
        Self {
            frontier: Frontier::with_seed(seed),
            job_tx,
            results_rx,
            in_flight: 0,
        }
    }

    pub(crate) async fn run(&mut self) -> anyhow::Result<()> {
        self.drain_queued()?;
        while let Some(result) = self.results_rx.recv().await {
            self.in_flight =
                self.in_flight.checked_sub(1).context("in_flight underflow")?;
            let parsed = match result {
                Ok(parsed) => parsed,
                Err(e) => {
                    warn!("crawl error: {e}");
                    self.drain_queued()?;
                    if self.in_flight == 0 && self.frontier.queued.is_empty() {
                        break;
                    }
                    continue;
                }
            };
            info!(
                "crawled {} -> {} outbound links",
                parsed.url.as_str(),
                parsed.outbound_links.len()
            );
            for link in parsed.outbound_links {
                let _ = self.frontier.try_enqueue(link.clone());
                debug!("\tqueued: {link}");
            }
            self.drain_queued()?;
            if self.in_flight == 0 && self.frontier.queued.is_empty() {
                break;
            }
        }

        Ok(())
    }

    fn drain_queued(&mut self) -> anyhow::Result<()> {
        while let Some(url) = self.frontier.queued.pop_front() {
            match self.job_tx.try_send(url) {
                Ok(()) => {
                    self.in_flight = self
                        .in_flight
                        .checked_add(1)
                        .context("in_flight overflow")?;
                }
                Err(async_channel::TrySendError::Full(url)) => {
                    self.frontier.queued.push_front(url);
                    break;
                }
                Err(async_channel::TrySendError::Closed(_)) => {
                    return Err(anyhow!("job channel closed"));
                }
            }
        }
        Ok(())
    }
}

async fn fetch_page(
    client: &reqwest::Client,
    url: Url,
) -> Result<RawPage, CrawlError> {
    let resp = client.get(url.as_str()).send().await?.error_for_status()?;
    let body = resp.text().await?;
    Ok(RawPage { url, content: body })
}

fn parse_page(raw: &RawPage) -> anyhow::Result<ParsedPage> {
    let document = scraper::Html::parse_document(&raw.content);
    let selector = Selector::parse("a").map_err(|e| anyhow!("{e}"))?;
    let outbound_links: Vec<Url> = document
        .select(&selector)
        .filter_map(|element| element.value().attr("href"))
        .filter_map(|href| raw.url.join(href).ok())
        .filter(|url| url.scheme() == "http" || url.scheme() == "https")
        // NOTE: same host name, so `en.wikipedia.org` entries don't have
        // `fr.wikipedia.org`.
        .filter(|url| url.host_str() == raw.url.host_str())
        .map(|mut url| {
            url.set_fragment(None);
            url
        })
        .filter(|url| *url != raw.url)
        .collect();
    Ok(ParsedPage { url: raw.url.clone(), outbound_links })
}

fn spawn_workers(
    num_workers: usize,
    job_rx: &async_channel::Receiver<Url>,
    results_tx: &mpsc::Sender<Result<ParsedPage, CrawlError>>,
    client: &reqwest::Client,
) {
    for _ in 0..num_workers {
        let rx = job_rx.clone();
        let tx = results_tx.clone();
        let client = client.clone();

        tokio::spawn(async move {
            while let Ok(url) = rx.recv().await {
                let result = match fetch_page(&client, url.clone()).await {
                    Ok(raw) => parse_page(&raw)
                        .map_err(|e| CrawlError::Parse(e.to_string())),
                    Err(e) => Err(e),
                };
                if tx.send(result).await.is_err() {
                    break;
                }
            }
        });
    }
}

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

    let client = ClientBuilder::new().user_agent(APP_USER_AGENT).build()?;

    let num_workers = args.workers.unwrap_or(
        available_parallelism()
            .with_context(|| "unable to get available_parallelism")?
            .into(),
    );

    let (job_tx, job_rx) = async_channel::bounded::<Url>(32);
    let (results_tx, results_rx) =
        mpsc::channel::<Result<ParsedPage, CrawlError>>(32);

    spawn_workers(num_workers, &job_rx, &results_tx, &client);
    drop(results_tx);

    let mut scheduler = Scheduler::new(seed, job_tx, results_rx);
    scheduler.run().await?;

    Ok(())
}
