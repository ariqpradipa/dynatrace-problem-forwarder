use std::sync::Arc;
use tokio::time::{sleep, Duration};
use futures::future::join_all;
use crate::config::Settings;
use crate::dynatrace::{DynatraceClient, Problem};
use crate::forwarder::Connector;
use crate::storage::{Database, ForwardedProblem, ForwardHistory};
use crate::error::Result;
use tracing::{info, error, debug};

pub struct ForwardingEngine {
    settings: Arc<Settings>,
    dynatrace_client: Arc<DynatraceClient>,
    connectors: Vec<Arc<Connector>>,
    database: Arc<Database>,
}

impl ForwardingEngine {
    /// Create a new forwarding engine
    pub async fn new(settings: Settings) -> Result<Self> {
        let dynatrace_client = Arc::new(DynatraceClient::new(&settings)?);
        
        let database = Arc::new(Database::new(&settings.database.path).await?);

        let mut connectors = Vec::new();
        for connector_config in &settings.connectors {
            let connector = Connector::new(connector_config.clone())?;
            connectors.push(Arc::new(connector));
        }

        Ok(Self {
            settings: Arc::new(settings),
            dynatrace_client,
            connectors,
            database,
        })
    }

    /// Start the polling loop
    pub async fn run(&self) -> Result<()> {
        info!("Starting Dynatrace Problem Forwarder...");
        info!("Polling interval: {}s", self.settings.polling.interval_seconds);
        info!("Configured connectors: {}", self.connectors.len());

        let interval = Duration::from_secs(self.settings.polling.interval_seconds);

        loop {
            if let Err(e) = self.poll_and_forward().await {
                error!("Error in polling cycle: {}", e);
            }

            debug!("Sleeping for {}s until next poll...", self.settings.polling.interval_seconds);
            sleep(interval).await;
        }
    }

    /// Poll Dynatrace and forward problems
    async fn poll_and_forward(&self) -> Result<()> {
        info!("Polling Dynatrace for problems...");

        let response = self.dynatrace_client.fetch_problems().await?;
        
        info!("Found {} problems to process", response.problems.len());

        let mut new_problems = 0;
        let mut status_changes = 0;
        let mut skipped = 0;

        for problem in response.problems {
            match self.process_problem(&problem).await {
                Ok(action) => {
                    match action {
                        ProcessAction::NewProblem => new_problems += 1,
                        ProcessAction::StatusChange => status_changes += 1,
                        ProcessAction::Skipped => skipped += 1,
                    }
                }
                Err(e) => {
                    error!("Error processing problem {}: {}", problem.problem_id, e);
                }
            }
        }

        info!(
            "Poll complete: {} new, {} status changes, {} skipped",
            new_problems, status_changes, skipped
        );

        Ok(())
    }

    /// Process a single problem
    async fn process_problem(&self, problem: &Problem) -> Result<ProcessAction> {
        debug!("Processing problem: {}", problem.summary());

        // Check if problem exists in database
        let db_problem = self.database.get_problem(&problem.problem_id).await?;

        match db_problem {
            None => {
                // New problem - forward it
                info!("New problem detected: {}", problem.summary());
                self.forward_to_connectors(problem).await?;
                
                // Insert into database
                let forwarded_problem = ForwardedProblem::new(
                    problem.problem_id.clone(),
                    problem.status.to_string(),
                    Some(problem.severity_level.clone()),
                    problem.title.clone(),
                );
                self.database.insert_problem(&forwarded_problem).await?;
                
                Ok(ProcessAction::NewProblem)
            }
            Some(db_record) if db_record.status != problem.status.to_string() => {
                // Status changed - forward update
                info!(
                    "Status change detected for {}: {} -> {}",
                    problem.problem_id,
                    db_record.status,
                    problem.status.to_string()
                );
                self.forward_to_connectors(problem).await?;
                
                // Update database
                self.database
                    .update_problem_status(&problem.problem_id, &problem.status.to_string())
                    .await?;
                
                Ok(ProcessAction::StatusChange)
            }
            Some(_) => {
                // No change - skip
                debug!("Problem {} unchanged, skipping", problem.problem_id);
                Ok(ProcessAction::Skipped)
            }
        }
    }

    /// Forward a problem to all connectors
    async fn forward_to_connectors(&self, problem: &Problem) -> Result<()> {
        debug!("Forwarding problem {} to {} connectors", problem.problem_id, self.connectors.len());

        // Forward to all connectors in parallel
        let forward_tasks: Vec<_> = self
            .connectors
            .iter()
            .map(|connector| {
                let connector = Arc::clone(connector);
                let problem = problem.clone();
                let database = Arc::clone(&self.database);
                
                async move {
                    let connector_name = connector.name().to_string();
                    match connector.forward_problem(&problem).await {
                        Ok(response) => {
                            info!(
                                "✓ Forwarded {} to '{}' (status: {})",
                                problem.problem_id,
                                connector_name,
                                response.status()
                            );
                            
                            // Record success in history
                            let history = ForwardHistory::new(
                                problem.problem_id.clone(),
                                connector_name,
                                "success".to_string(),
                                Some(response.status().as_u16() as i32),
                                None,
                            );
                            let _ = database.insert_forward_history(&history).await;
                        }
                        Err(e) => {
                            error!(
                                "✗ Failed to forward {} to '{}': {}",
                                problem.problem_id, connector_name, e
                            );
                            
                            // Record failure in history
                            let history = ForwardHistory::new(
                                problem.problem_id.clone(),
                                connector_name,
                                "failed".to_string(),
                                None,
                                Some(e.to_string()),
                            );
                            let _ = database.insert_forward_history(&history).await;
                        }
                    }
                }
            })
            .collect();

        join_all(forward_tasks).await;

        Ok(())
    }

    /// Get reference to database (for CLI commands)
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Get reference to Dynatrace client (for CLI commands)
    pub fn dynatrace_client(&self) -> &DynatraceClient {
        &self.dynatrace_client
    }

    /// Get reference to connectors (for CLI commands)
    pub fn connectors(&self) -> &[Arc<Connector>] {
        &self.connectors
    }
}

#[derive(Debug, PartialEq)]
enum ProcessAction {
    NewProblem,
    StatusChange,
    Skipped,
}
