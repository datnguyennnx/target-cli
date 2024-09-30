use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Parser, Debug, Serialize, Deserialize)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// URL to test. Either this or --sitemap must be provided
    #[arg(long)]
    pub url: Option<String>,

    /// Path to sitemap XML file. Either this or --url must be provided
    #[arg(long)]
    pub sitemap: Option<String>,

    /// Number of requests to send (ignored if --stress is set)
    #[arg(short, long, default_value_t = 100)]
    pub requests: u32,

    /// Number of concurrent requests
    #[arg(short, long, default_value_t = 10)]
    pub concurrency: u32,

    /// Enable stress test mode (runs for a specified duration instead of a fixed number of requests)
    #[arg(short, long)]
    pub stress: bool,

    /// Duration of the stress test in seconds (only used if --stress is set)
    #[arg(short, long, default_value_t = 60)]
    pub duration: u64,

    /// Collect and display resource usage data for 60 seconds
    #[arg(long)]
    pub resource_usage: bool,

    /// Path to JSON configuration file
    #[arg(long)]
    pub config: Option<String>,
}

impl Args {
    /// Validate the arguments to ensure either URL or sitemap is provided
    pub fn validate(&self) -> Result<(), String> {
        if self.resource_usage {
            return Ok(());
        }
        match (&self.url, &self.sitemap) {
            (None, None) => Err("Either --url or --sitemap must be provided".to_string()),
            (Some(_), Some(_)) => Err("Only one of --url or --sitemap should be provided".to_string()),
            (Some(url), None) => {
                if url.starts_with("http://") || url.starts_with("https://") {
                    Ok(())
                } else {
                    Err("URL must start with http:// or https://".to_string())
                }
            },
            (None, Some(_)) => Ok(()),
        }
    }

    /// Parse JSON configuration file and return Args
    pub fn from_json(path: &Path) -> Result<Self, String> {
        let config_str = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        
        let args: Args = serde_json::from_str(&config_str)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;
        
        args.validate()?;
        Ok(args)
    }
}