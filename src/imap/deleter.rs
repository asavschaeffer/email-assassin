use crate::error::AppError;
use crate::imap::provider::ImapProvider;
use futures::StreamExt;

use super::connect_imap;

pub async fn nuke_sender(
    email: &str,
    password: &str,
    folder: &str,
    sender: &str,
    use_trash: bool,
) -> Result<usize, AppError> {
    let mut session = connect_imap(email, password, folder).await?;
    let provider = ImapProvider::from_email(email);

    // Sanitize sender to prevent malformed IMAP search queries
    let sanitized_sender = sender.replace('"', "");
    let search_query = format!("FROM \"{}\"", sanitized_sender);
    let uids = session
        .uid_search(&search_query)
        .await
        .map_err(|e| AppError::Imap(e.to_string()))?;

    let uid_vec: Vec<u32> = uids.into_iter().collect();
    let total = uid_vec.len();

    if total == 0 {
        if let Err(e) = session.logout().await {
            tracing::warn!("Failed to logout after empty search: {}", e);
        }
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

    if let Err(e) = session.logout().await {
        tracing::warn!("Failed to logout after deleting: {}", e);
    }
    Ok(total)
}
