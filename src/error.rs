use thiserror::Error;

#[derive(Error, Debug)]
pub enum ForwarderError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Dynatrace API error: {0}")]
    DynatraceApi(#[from] reqwest::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Connector error: {connector}: {message}")]
    Connector {
        connector: String,
        message: String,
    },

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ForwarderError>;
