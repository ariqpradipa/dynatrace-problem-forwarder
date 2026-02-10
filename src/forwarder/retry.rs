use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// Retry an operation with exponential backoff
pub async fn retry_with_backoff<F, T, E>(
    operation_name: &str,
    max_attempts: u32,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>,
{
    let mut attempt = 0;

    loop {
        attempt += 1;
        
        match operation().await {
            Ok(result) => {
                if attempt > 1 {
                    debug!(
                        "Operation '{}' succeeded on attempt {}/{}",
                        operation_name, attempt, max_attempts
                    );
                }
                return Ok(result);
            }
            Err(_e) if attempt < max_attempts => {
                let delay_secs = 2_u64.pow(attempt - 1);
                warn!(
                    "Operation '{}' failed (attempt {}/{}), retrying in {}s...",
                    operation_name, attempt, max_attempts, delay_secs
                );
                sleep(Duration::from_secs(delay_secs)).await;
            }
            Err(e) => {
                warn!(
                    "Operation '{}' failed after {} attempts",
                    operation_name, max_attempts
                );
                return Err(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_success_first_attempt() {
        let mut call_count = 0;
        
        let result = retry_with_backoff("test", 3, || {
            call_count += 1;
            Box::pin(async { Ok::<_, String>(42) })
        })
        .await;

        assert_eq!(result, Ok(42));
        assert_eq!(call_count, 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let mut call_count = 0;
        
        let result = retry_with_backoff("test", 3, || {
            call_count += 1;
            Box::pin(async move {
                if call_count < 3 {
                    Err("temporary failure")
                } else {
                    Ok(42)
                }
            })
        })
        .await;

        assert_eq!(result, Ok(42));
        assert_eq!(call_count, 3);
    }

    #[tokio::test]
    async fn test_retry_all_failures() {
        let mut call_count = 0;
        
        let result = retry_with_backoff("test", 3, || {
            call_count += 1;
            Box::pin(async { Err::<i32, _>("permanent failure") })
        })
        .await;

        assert_eq!(result, Err("permanent failure"));
        assert_eq!(call_count, 3);
    }
}
