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

    /// Fetch problems from Dynatrace API (handles pagination automatically)
    #[allow(unused_assignments)]
    pub async fn fetch_problems(&self) -> Result<ProblemsResponse> {
        debug!("Fetching problems from: {}", self.problems_url);

        let mut all_problems = Vec::new();
        let mut next_page_key: Option<String> = None;
        let mut page_num = 1;
        // Initialize to 0, will be set from API response on first iteration
        let mut total_count = 0;

        loop {
            // Build URL with pagination key if available
            let url = if let Some(ref page_key) = next_page_key {
                format!("{}&nextPageKey={}", self.problems_url, page_key)
            } else {
                self.problems_url.clone()
            };

            debug!("Fetching page {} from Dynatrace...", page_num);

            let response = self
                .client
                .get(&url)
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

            let mut problems_response = response.json::<ProblemsResponse>().await?;

            debug!(
                "Fetched page {} with {} problems (page size: {})",
                page_num,
                problems_response.problems.len(),
                problems_response.page_size
            );

            total_count = problems_response.total_count;
            all_problems.append(&mut problems_response.problems);

            // Check if there are more pages
            if problems_response.next_page_key.is_some() {
                next_page_key = problems_response.next_page_key;
                page_num += 1;
            } else {
                break;
            }
        }

        info!(
            "Fetched {} problems from Dynatrace across {} page(s) (total count: {})",
            all_problems.len(),
            page_num,
            total_count
        );

        Ok(ProblemsResponse {
            total_count,
            page_size: all_problems.len() as i32,
            problems: all_problems,
            next_page_key: None,
        })
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
