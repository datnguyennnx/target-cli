use clap::Parser;
use std::sync::Arc;
use tokio::time::Instant;
use std::sync::atomic::AtomicBool;
use tokio::sync::Mutex;
use std::path::Path;
use std::time::Duration;

mod args;
mod metrics;
mod http_client;
mod utils;
mod structure_output;
mod resource_monitor;

use args::Args;
use metrics::Metrics;
use http_client::{load_test, stress_test, TestConfig};
use utils::{parse_sitemap, format_duration, UtilError};
use structure_output::print_test_results;
use resource_monitor::ResourceMonitor;

#[derive(Debug)]
enum AppError {
    ArgValidation(String),
    NoUrls,
    Util(UtilError),
    Other(Box<dyn std::error::Error>),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::ArgValidation(msg) => write!(f, "Argument validation error: {}", msg),
            AppError::NoUrls => write!(f, "No valid URLs found to test"),
            AppError::Util(e) => write!(f, "Utility error: {}", e),
            AppError::Other(e) => write!(f, "Error: {}", e),
        }
    }
}

impl std::error::Error for AppError {}

impl From<UtilError> for AppError {
    fn from(error: UtilError) -> Self {
        AppError::Util(error)
    }
}

async fn parse_arguments() -> Result<Args, AppError> {
    let args = Args::parse();
    if let Some(config_path) = args.config() {
        Args::from_json(Path::new(config_path)).map_err(|e| AppError::ArgValidation(e))
    } else {
        args.validate().map_err(AppError::ArgValidation)?;
        Ok(args)
    }
}

fn prepare_urls(args: &Args) -> Result<Arc<Vec<String>>, AppError> {
    let urls = if let Some(sitemap_path) = args.sitemap() {
        parse_sitemap(sitemap_path)?
    } else if let Some(url) = args.url() {
        vec![url.clone()]
    } else {
        return Err(AppError::ArgValidation("Either a sitemap or a URL must be provided".into()));
    };

    if urls.is_empty() {
        Err(AppError::NoUrls)
    } else {
        Ok(Arc::new(urls))
    }
}

async fn run_test(config: TestConfig, metrics: Arc<Mutex<Metrics>>, is_finished: Arc<AtomicBool>) {
    if let Some(duration) = config.duration {
        stress_test(config.urls, config.concurrency, duration, metrics, is_finished).await;
    } else {
        load_test(config.urls, config.total_requests, config.concurrency, metrics, is_finished).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = parse_arguments().await?;
    let is_finished = Arc::new(AtomicBool::new(false));

    if args.resource_usage() {
        println!("Collecting resource usage data for 60 seconds");
        let resource_monitor = ResourceMonitor::new(Arc::clone(&is_finished));
        let resource_monitor_handle = tokio::spawn(resource_monitor.start());
        tokio::time::sleep(Duration::from_secs(60)).await;
        is_finished.store(true, std::sync::atomic::Ordering::SeqCst);
        let (cpu_samples, memory_samples, network_samples) = resource_monitor_handle.await.map_err(|e| AppError::Other(Box::new(e)))?;
        print_test_results(None, None, &cpu_samples, &memory_samples, &network_samples);
    } else {
        let urls = prepare_urls(&args)?;
        let metrics = Arc::new(Mutex::new(Metrics::new()));

        println!("\n{}", if args.stress() { "Stress Test" } else { "Load Test" });
        println!("URLs to test: {}", urls.len());
        println!("Concurrency: {}", args.concurrency());
        if args.stress() {
            println!("Duration: {}", format_duration(std::time::Duration::from_secs(args.duration())));
        } else {
            println!("Total requests: {}", args.requests());
        }
        println!();

        let start = Instant::now();

        let config = TestConfig {
            urls: Arc::clone(&urls),
            concurrency: args.concurrency(),
            total_requests: args.requests(),
            duration: if args.stress() { Some(args.duration()) } else { None },
        };

        let test_handle = tokio::spawn(run_test(config, Arc::clone(&metrics), Arc::clone(&is_finished)));

        let resource_monitor = ResourceMonitor::new(Arc::clone(&is_finished));
        let resource_monitor_handle = tokio::spawn(resource_monitor.start());

        test_handle.await.map_err(|e| AppError::Other(Box::new(e)))?;
        let (cpu_samples, memory_samples, network_samples) = resource_monitor_handle.await.map_err(|e| AppError::Other(Box::new(e)))?;
        let total_duration = start.elapsed();

        let metrics = metrics.lock().await;
        let summary = metrics.summary();

        print_test_results(Some(&summary), Some(total_duration), &cpu_samples, &memory_samples, &network_samples);
    }

    Ok(())
}
