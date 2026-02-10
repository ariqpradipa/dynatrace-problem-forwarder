use reqwest::{Client, Response};
use serde_json::json;
use std::time::Duration;
use crate::config::{ConnectorConfig, HttpMethod};
use crate::dynatrace::Problem;
use crate::error::{ForwarderError, Result};
use crate::forwarder::retry::retry_with_backoff;
use tracing::{debug, info, error, warn};

pub struct Connector {
    client: Client,
    config: ConnectorConfig,
}

impl Connector {
    /// Create a new connector
    pub fn new(config: ConnectorConfig) -> Result<Self> {
        let timeout = Duration::from_secs(config.timeout_seconds.unwrap_or(30));

        // Warn if SSL verification is disabled
        if !config.verify_ssl {
            warn!(
                "⚠️  SSL verification disabled for connector '{}'. This should only be used for testing!",
                config.name
            );
        }

        let client = Client::builder()
            .timeout(timeout)
            .danger_accept_invalid_certs(!config.verify_ssl)
            .build()?;

        Ok(Self { client, config })
    }

    /// Forward a problem to the connector
    pub async fn forward_problem(&self, problem: &Problem) -> Result<Response> {
        let max_attempts = self.config.retry_attempts.unwrap_or(3);

        let connector_name = self.config.name.clone();
        let url = self.config.url.clone();
        let method = self.config.method.clone();
        let headers = self.config.headers.clone();
        let client = self.client.clone();

        let result = retry_with_backoff(
            &format!("forward to {}", connector_name),
            max_attempts,
            move || {
                let connector_name = connector_name.clone();
                let url = url.clone();
                let method = method.clone();
                let headers = headers.clone();
                let client = client.clone();
                let problem = problem.clone();

                Box::pin(async move {
                    Self::send_request(&client, &url, &method, headers.as_ref(), &problem).await
                        .map_err(|e| {
                            ForwarderError::Connector {
                                connector: connector_name.clone(),
                                message: e.to_string(),
                            }
                        })
                })
            },
        )
        .await?;

        Ok(result)
    }

    /// Forward multiple problems to the connector in a single batch request
    pub async fn forward_problems_batch(&self, problems: &[Problem]) -> Result<Response> {
        let max_attempts = self.config.retry_attempts.unwrap_or(3);

        let connector_name = self.config.name.clone();
        let url = self.config.url.clone();
        let method = self.config.method.clone();
        let headers = self.config.headers.clone();
        let client = self.client.clone();
        let problems = problems.to_vec();

        let result = retry_with_backoff(
            &format!("forward batch to {}", connector_name),
            max_attempts,
            move || {
                let connector_name = connector_name.clone();
                let url = url.clone();
                let method = method.clone();
                let headers = headers.clone();
                let client = client.clone();
                let problems = problems.clone();

                Box::pin(async move {
                    Self::send_batch_request(&client, &url, &method, headers.as_ref(), &problems).await
                        .map_err(|e| {
                            ForwarderError::Connector {
                                connector: connector_name.clone(),
                                message: e.to_string(),
                            }
                        })
                })
            },
        )
        .await?;

        Ok(result)
    }

    /// Send HTTP request with problem payload
    async fn send_request(
        client: &Client,
        url: &str,
        method: &HttpMethod,
        headers: Option<&std::collections::HashMap<String, String>>,
        problem: &Problem,
    ) -> Result<Response> {
        debug!("Sending problem {} to {}", problem.problem_id, url);

        // Build the request
        let mut request = match method {
            HttpMethod::Post => client.post(url),
            HttpMethod::Put => client.put(url),
            HttpMethod::Patch => client.patch(url),
            HttpMethod::Get => client.get(url),
        };

        // Add custom headers
        if let Some(headers_map) = headers {
            for (key, value) in headers_map {
                request = request.header(key, value);
            }
        }

        // Add JSON body (serialize the problem)
        let payload = json!(problem);
        request = request.json(&payload);

        // Send request
        let response = request.send().await?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!(
                "Connector returned error ({}): {}",
                status, error_text
            );
            return Err(ForwarderError::Connector {
                connector: url.to_string(),
                message: format!("HTTP {}: {}", status, error_text),
            });
        }

        debug!("Successfully forwarded problem {} (status: {})", problem.problem_id, status);

        Ok(response)
    }

    /// Send HTTP request with multiple problems as array payload
    async fn send_batch_request(
        client: &Client,
        url: &str,
        method: &HttpMethod,
        headers: Option<&std::collections::HashMap<String, String>>,
        problems: &[Problem],
    ) -> Result<Response> {
        debug!("Sending batch of {} problems to {}", problems.len(), url);

        // Build the request
        let mut request = match method {
            HttpMethod::Post => client.post(url),
            HttpMethod::Put => client.put(url),
            HttpMethod::Patch => client.patch(url),
            HttpMethod::Get => client.get(url),
        };

        // Add custom headers
        if let Some(headers_map) = headers {
            for (key, value) in headers_map {
                request = request.header(key, value);
            }
        }

        // Add JSON body (array of problems)
        let payload = json!(problems);
        request = request.json(&payload);

        // Send request
        let response = request.send().await?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!(
                "Connector returned error ({}): {}",
                status, error_text
            );
            return Err(ForwarderError::Connector {
                connector: url.to_string(),
                message: format!("HTTP {}: {}", status, error_text),
            });
        }

        debug!("Successfully forwarded batch of {} problems (status: {})", problems.len(), status);

        Ok(response)
    }

    /// Test the connector with a dummy payload
    pub async fn test(&self) -> Result<()> {
        info!("Testing connector '{}'...", self.config.name);

        let test_problem = Problem {
            problem_id: "TEST-12345".to_string(),
            display_id: "P-TEST".to_string(),
            title: "Test problem from dynatrace-problem-forwarder".to_string(),
            impact_level: "INFRASTRUCTURE".to_string(),
            severity_level: "CUSTOM_ALERT".to_string(),
            status: crate::dynatrace::ProblemStatus::Open,
            affected_entities: vec![],
            impacted_entities: vec![],
            root_cause_entity: None,
            management_zones: vec![],
            entity_tags: vec![],
            problem_filters: vec![],
            start_time: chrono::Utc::now().timestamp_millis(),
            end_time: -1,
        };

        let response = self.forward_problem(&test_problem).await?;
        
        info!(
            "✓ Connector '{}' test successful (status: {})",
            self.config.name,
            response.status()
        );

        Ok(())
    }

    /// Get the connector name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Check if connector is in batch mode
    pub fn is_batch_mode(&self) -> bool {
        self.config.batch_mode
    }
}
