use std::sync::Arc;
use tokio::time::{sleep, Duration};
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
        let mut problems_to_forward = Vec::new();

        // Collect problems that need forwarding
        for problem in response.problems {
            match self.check_problem(&problem).await {
                Ok(action) => {
                    match action {
                        ProcessAction::NewProblem => {
                            new_problems += 1;
                            problems_to_forward.push(problem);
                        }
                        ProcessAction::StatusChange => {
                            status_changes += 1;
                            problems_to_forward.push(problem);
                        }
                        ProcessAction::Skipped => skipped += 1,
                    }
                }
                Err(e) => {
                    error!("Error processing problem {}: {}", problem.problem_id, e);
                }
            }
        }

        // Forward collected problems (batch or individual depending on connector config)
        if !problems_to_forward.is_empty() {
            if let Err(e) = self.forward_collected_problems(&problems_to_forward).await {
                error!("Error forwarding problems: {}", e);
            }
        }

        info!(
            "Poll complete: {} new, {} status changes, {} skipped",
            new_problems, status_changes, skipped
        );

        Ok(())
    }

    /// Check if a problem needs forwarding and update database
    async fn check_problem(&self, problem: &Problem) -> Result<ProcessAction> {
        debug!("Processing problem: {}", problem.summary());

        // Check if problem exists in database
        let db_problem = self.database.get_problem(&problem.problem_id).await?;

        match db_problem {
            None => {
                // New problem - will forward it
                info!("New problem detected: {}", problem.summary());

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
                // Status changed - will forward update
                info!(
                    "Status change detected for {}: {} -> {}",
                    problem.problem_id,
                    db_record.status,
                    problem.status.to_string()
                );

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

    /// Forward collected problems to all connectors (batch or individual based on connector config)
    async fn forward_collected_problems(&self, problems: &[Problem]) -> Result<()> {
        info!("Forwarding {} problems to connectors", problems.len());

        // Group connectors by batch mode
        let (batch_connectors, individual_connectors): (Vec<_>, Vec<_>) = self
            .connectors
            .iter()
            .partition(|c| c.is_batch_mode());

        let mut forward_tasks = Vec::new();

        // Batch mode connectors - send all problems in one request
        for connector in batch_connectors {
            let connector = Arc::clone(connector);
            let problems = problems.to_vec();
            let database = Arc::clone(&self.database);

            let task = tokio::spawn(async move {
                let connector_name = connector.name().to_string();
                match connector.forward_problems_batch(&problems).await {
                    Ok(response) => {
                        info!(
                            "✓ Forwarded batch of {} problems to '{}' (status: {})",
                            problems.len(),
                            connector_name,
                            response.status()
                        );

                        // Record success in history for each problem
                        for problem in &problems {
                            let history = ForwardHistory::new(
                                problem.problem_id.clone(),
                                connector_name.clone(),
                                "success".to_string(),
                                Some(response.status().as_u16() as i32),
                                None,
                            );
                            let _ = database.insert_forward_history(&history).await;
                        }
                    }
                    Err(e) => {
                        error!(
                            "✗ Failed to forward batch to '{}': {}",
                            connector_name, e
                        );

                        // Record failure in history for each problem
                        for problem in &problems {
                            let history = ForwardHistory::new(
                                problem.problem_id.clone(),
                                connector_name.clone(),
                                "failed".to_string(),
                                None,
                                Some(e.to_string()),
                            );
                            let _ = database.insert_forward_history(&history).await;
                        }
                    }
                }
            });
            forward_tasks.push(task);
        }

        // Individual mode connectors - send each problem separately
        for connector in individual_connectors {
            for problem in problems {
                let connector = Arc::clone(connector);
                let problem = problem.clone();
                let database = Arc::clone(&self.database);

                let task = tokio::spawn(async move {
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
                });
                forward_tasks.push(task);
            }
        }

        // Wait for all tasks to complete
        for task in forward_tasks {
            let _ = task.await;
        }

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
