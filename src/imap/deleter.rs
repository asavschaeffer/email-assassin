use crate::error::AppError;
use crate::imap::provider::ImapProvider;
use futures::StreamExt;

use super::connect_imap;

/// Maximum UIDs per IMAP command. Keeps individual commands under typical
/// server command-length limits and avoids long-running single operations.
const DELETE_CHUNK_SIZE: usize = 1000;

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
    let search_query = format!("FROM \"{sanitized_sender}\"");
    let uids = session
        .uid_search(&search_query)
        .await
        .map_err(|e| AppError::Imap(e.to_string()))?;

    let uid_vec: Vec<u32> = uids.into_iter().collect();
    let total = uid_vec.len();

    if total == 0 {
        if let Err(e) = session.logout().await {
            tracing::warn!(error = %e, "logout failed after empty search");
        }
        return Ok(0);
    }

    for chunk in uid_vec.chunks(DELETE_CHUNK_SIZE) {
        let uid_str = chunk
            .iter()
            .map(std::string::ToString::to_string)
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
        tracing::warn!(error = %e, "logout failed after deletion");
    }
    Ok(total)
}
