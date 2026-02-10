use clap::Parser;
use dynatrace_problem_forwarder::{
    cli::{Cli, Commands},
    config::Settings,
    forwarder::ForwardingEngine,
};
use std::io::{self, Write};
use tracing::{info, error};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() {
    // Parse CLI arguments
    let cli = Cli::parse();

    match run(cli).await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Run { config, nohup } => {
            // If nohup flag is set, start in background
            if nohup {
                dynatrace_problem_forwarder::utils::start_background(&config)?;
                return Ok(());
            }

            // Otherwise, run in foreground
            // Load configuration
            let settings = Settings::load(&config)?;

            // Initialize logging
            init_logging(&settings);

            info!("Dynatrace Problem Forwarder v{}", env!("CARGO_PKG_VERSION"));
            info!("Configuration loaded from: {}", config.display());

            // Create forwarding engine
            let engine = ForwardingEngine::new(settings).await?;

            // Setup graceful shutdown
            let shutdown_handle = tokio::spawn(dynatrace_problem_forwarder::utils::setup_shutdown_handler());

            // Run the engine in a separate task
            let _engine_handle = tokio::spawn(async move {
                if let Err(e) = engine.run().await {
                    error!("Engine error: {}", e);
                }
            });

            // Wait for shutdown signal
            shutdown_handle.await?;

            info!("Shutdown complete");
        }

        Commands::ClearCache { config, confirm } => {
            let settings = Settings::load(&config)?;
            init_logging(&settings);

            info!("Clear Cache Command");

            // Confirm operation
            if !confirm {
                print!("This will delete all cached problems. Are you sure? (y/N): ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    info!("Operation cancelled");
                    return Ok(());
                }
            }

            let engine = ForwardingEngine::new(settings).await?;
            let count = engine.database().clear_all_problems().await?;

            info!("✓ Cleared {} problems from cache", count);
            println!("Cleared {} problems from cache", count);
        }

        Commands::TestDynatrace { config } => {
            let settings = Settings::load(&config)?;
            init_logging(&settings);

            info!("Testing Dynatrace API connectivity...");

            let engine = ForwardingEngine::new(settings).await?;
            engine.dynatrace_client().test_connection().await?;

            println!("✓ Dynatrace API connection successful");
        }

        Commands::TestConnectors { config } => {
            let settings = Settings::load(&config)?;
            init_logging(&settings);

            info!("Testing connector configurations...");

            let engine = ForwardingEngine::new(settings).await?;

            for connector in engine.connectors() {
                match connector.test().await {
                    Ok(_) => {
                        println!("✓ Connector '{}' test successful", connector.name());
                    }
                    Err(e) => {
                        println!("✗ Connector '{}' test failed: {}", connector.name(), e);
                    }
                }
            }
        }

        Commands::Stats { config } => {
            let settings = Settings::load(&config)?;
            init_logging(&settings);

            info!("Fetching database statistics...");

            let engine = ForwardingEngine::new(settings).await?;
            let stats = engine.database().get_stats().await?;

            println!("\n=== Database Statistics ===");
            println!("Total problems tracked:  {}", stats.total_problems);
            println!("  Open problems:         {}", stats.open_problems);
            println!("  Closed problems:       {}", stats.closed_problems);
            println!("\nForward history:");
            println!("  Total forwards:        {}", stats.total_forwards);
            println!("  Successful:            {}", stats.successful_forwards);
            println!("  Failed:                {}", stats.failed_forwards);
            println!();
        }

        Commands::Stop { config } => {
            dynatrace_problem_forwarder::utils::stop_background(&config)?;
        }
    }

    Ok(())
}

fn init_logging(settings: &Settings) {
    let log_level = settings.logging.level.as_str();
    let log_format = settings.logging.format.as_str();

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    match log_format {
        "json" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().json())
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().pretty())
                .init();
        }
    }
}

