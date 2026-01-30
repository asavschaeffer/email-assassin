pub mod deleter;
pub mod provider;
pub mod scanner;

use crate::error::AppError;
use provider::ImapProvider;
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn connect_imap(
    email: &str,
    password: &str,
    folder: &str,
) -> Result<async_imap::Session<async_native_tls::TlsStream<async_std::net::TcpStream>>, AppError>
{
    let provider = ImapProvider::from_email(email);
    let tls = async_native_tls::TlsConnector::new();
    let tcp = async_std::future::timeout(
        CONNECT_TIMEOUT,
        async_std::net::TcpStream::connect((provider.host, provider.port)),
    )
    .await
    .map_err(|_| AppError::Connection("TCP connect timed out after 30s".to_string()))?
    .map_err(|e| AppError::Connection(e.to_string()))?;

    let tls_stream = tls
        .connect(provider.host, tcp)
        .await
        .map_err(|e| AppError::Tls(e.to_string()))?;

    let client = async_imap::Client::new(tls_stream);
    let mut session = client
        .login(email, password)
        .await
        .map_err(|(e, _)| AppError::Auth(e.to_string()))?;

    session
        .select(folder)
        .await
        .map_err(|e| AppError::Imap(e.to_string()))?;

    Ok(session)
}
