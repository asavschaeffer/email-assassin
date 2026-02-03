pub mod deleter;
pub mod provider;
pub mod scanner;

use crate::error::AppError;
use futures::StreamExt;
use provider::ImapProvider;
use std::time::Duration;

/// TCP connect timeout. 30s is generous enough for high-latency networks
/// while still failing fast on unreachable hosts.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

type ImapSession = async_imap::Session<async_native_tls::TlsStream<async_std::net::TcpStream>>;

/// Creates a new IMAP session without selecting a folder.
/// Extracted to avoid code duplication between `connect_imap` and `list_folders`.
async fn create_session(email: &str, password: &str) -> Result<ImapSession, AppError> {
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
    let session = client
        .login(email, password)
        .await
        .map_err(|(e, _)| AppError::Auth(e.to_string()))?;

    Ok(session)
}

pub async fn connect_imap(
    email: &str,
    password: &str,
    folder: &str,
) -> Result<ImapSession, AppError> {
    let mut session = create_session(email, password).await?;

    session
        .select(folder)
        .await
        .map_err(|e| AppError::Imap(e.to_string()))?;

    Ok(session)
}

pub async fn list_folders(email: &str, password: &str) -> Result<Vec<String>, AppError> {
    let mut session = create_session(email, password).await?;

    let names: Vec<String> = session
        .list(Some(""), Some("*"))
        .await
        .map_err(|e| AppError::Imap(e.to_string()))?
        .filter_map(|result| async { result.ok().map(|name| name.name().to_string()) })
        .collect()
        .await;

    if let Err(e) = session.logout().await {
        tracing::warn!(error = %e, "failed to logout IMAP session");
    }

    let mut folders = names;
    folders.sort_by(|a, b| match (a.eq_ignore_ascii_case("INBOX"), b.eq_ignore_ascii_case("INBOX")) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.cmp(b),
    });

    Ok(folders)
}
