use reqwest::{Client, header};
use crate::config::Settings;
use crate::dynatrace::models::ProblemsResponse;
use crate::error::{ForwarderError, Result};
use tracing::{debug, info, warn};

pub struct DynatraceClient {
    client: Client,
    api_token: String,
    problems_url: String,
}

impl DynatraceClient {
    /// Create a new Dynatrace client
    pub fn new(settings: &Settings) -> Result<Self> {
        let api_token = settings
            .dynatrace
            .api_token
            .clone()
            .ok_or_else(|| ForwarderError::Config("Missing DYNATRACE_API_TOKEN".to_string()))?;

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let problems_url = settings.get_problems_url();

        Ok(Self {
            client,
            api_token,
            problems_url,
        })
    }

    /// Fetch problems from Dynatrace API
    pub async fn fetch_problems(&self) -> Result<ProblemsResponse> {
        debug!("Fetching problems from: {}", self.problems_url);

        let response = self
            .client
            .get(&self.problems_url)
            .header(header::AUTHORIZATION, format!("Api-Token {}", self.api_token))
            .header(header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            warn!("Dynatrace API returned error ({}): {}", status, error_text);
            return Err(ForwarderError::Config(format!(
                "Dynatrace API error ({}): {}",
                status, error_text
            )));
        }

        let problems_response = response.json::<ProblemsResponse>().await?;
        
        info!(
            "Fetched {} problems from Dynatrace (total count: {})",
            problems_response.problems.len(),
            problems_response.total_count
        );

        Ok(problems_response)
    }

    /// Test connectivity to Dynatrace API
    pub async fn test_connection(&self) -> Result<()> {
        info!("Testing Dynatrace API connectivity...");
        
        let response = self.fetch_problems().await?;
        
        info!(
            "✓ Successfully connected to Dynatrace API. Found {} problems.",
            response.total_count
        );
        
        Ok(())
    }

    /// Get the base URL for display/logging
    pub fn base_url(&self) -> &str {
        &self.problems_url
    }
}
