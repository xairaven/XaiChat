use crate::config::ConfigError;
use crate::logs::LogsError;
use crate::ui::GraphicsBackendError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Configuration. {0}")]
    Config(#[from] ConfigError),

    #[error("Graphics Backend. {0}")]
    GraphicsBackend(#[from] GraphicsBackendError),

    #[error("Logger. {0}")]
    Logs(#[from] LogsError),
}
