use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dtpf")]
#[command(about = "Forward Dynatrace problems to external systems", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the forwarder service
    Run {
        /// Path to configuration file
        #[arg(short, long, env = "CONFIG_PATH", default_value = "./config.yaml")]
        config: PathBuf,

        /// Run in background using nohup
        #[arg(long)]
        nohup: bool,
    },

    /// Clear the cache database (re-forward all open problems)
    ClearCache {
        /// Path to configuration file
        #[arg(short, long, env = "CONFIG_PATH", default_value = "./config.yaml")]
        config: PathBuf,

        /// Confirm the operation without prompting
        #[arg(long)]
        confirm: bool,
    },

    /// Test connectivity to Dynatrace API
    TestDynatrace {
        /// Path to configuration file
        #[arg(short, long, env = "CONFIG_PATH", default_value = "./config.yaml")]
        config: PathBuf,
    },

    /// Test forwarding to connectors (sends a test payload)
    TestConnectors {
        /// Path to configuration file
        #[arg(short, long, env = "CONFIG_PATH", default_value = "./config.yaml")]
        config: PathBuf,
    },

    /// Show current database statistics
    Stats {
        /// Path to configuration file
        #[arg(short, long, env = "CONFIG_PATH", default_value = "./config.yaml")]
        config: PathBuf,
    },

    /// Stop the background forwarder service
    Stop {
        /// Path to configuration file (used to locate PID file)
        #[arg(short, long, env = "CONFIG_PATH", default_value = "./config.yaml")]
        config: PathBuf,
    },
}
