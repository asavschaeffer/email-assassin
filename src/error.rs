use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IMAP error: {0}")]
    Imap(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Connection failed: {0}")]
    Connection(String),
}

impl From<async_imap::error::Error> for AppError {
    fn from(e: async_imap::error::Error) -> Self {
        AppError::Imap(e.to_string())
    }
}

impl From<async_native_tls::Error> for AppError {
    fn from(e: async_native_tls::Error) -> Self {
        AppError::Tls(e.to_string())
    }
}
