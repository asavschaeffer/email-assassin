use crate::error::AppError;
use crate::imap::provider::ImapProvider;
use futures::StreamExt;

async fn connect_imap(
    email: &str,
    password: &str,
    folder: &str,
) -> Result<async_imap::Session<async_native_tls::TlsStream<async_std::net::TcpStream>>, AppError> {
    let provider = ImapProvider::from_email(email);
    let tls = async_native_tls::TlsConnector::new();
    let tcp = async_std::net::TcpStream::connect((provider.host, provider.port))
        .await
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

pub async fn nuke_sender(
    email: &str,
    password: &str,
    folder: &str,
    sender: &str,
    use_trash: bool,
) -> Result<usize, AppError> {
    let mut session = connect_imap(email, password, folder).await?;
    let provider = ImapProvider::from_email(email);

    // Search for all emails from this sender
    let search_query = format!("FROM \"{}\"", sender);
    let uids = session
        .uid_search(&search_query)
        .await
        .map_err(|e| AppError::Imap(e.to_string()))?;

    let uid_vec: Vec<u32> = uids.into_iter().collect();
    let total = uid_vec.len();

    if total == 0 {
        session.logout().await.ok();
        return Ok(0);
    }

    // Process in chunks of 1000
    let chunk_size = 1000;
    for chunk in uid_vec.chunks(chunk_size) {
        let uid_str = chunk
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        if use_trash {
            session
                .uid_mv(&uid_str, provider.trash_folder)
                .await
                .map_err(|e| AppError::Imap(e.to_string()))?;
        } else {
            session
                .uid_store(&uid_str, "+FLAGS (\\Deleted)")
                .await
                .map_err(|e| AppError::Imap(e.to_string()))?
                .collect::<Vec<_>>()
                .await;

            session
                .expunge()
                .await
                .map_err(|e| AppError::Imap(e.to_string()))?
                .collect::<Vec<_>>()
                .await;
        }
    }

    session.logout().await.ok();
    Ok(total)
}
