use crate::error::AppError;
use crate::state::SenderInfo;
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;

use super::connect_imap;

static FROM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)From:\s*(.*)").unwrap());
static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<([^>]+)>").unwrap());

const MAX_CONCURRENT: usize = 10;

fn parse_sender(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw);
    if let Some(m) = FROM_RE.captures(&text) {
        let raw_from = m.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        if let Some(email_match) = EMAIL_RE.captures(raw_from) {
            return email_match
                .get(1)
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_else(|| "unknown".to_string());
        }
        if !raw_from.is_empty() {
            return raw_from.to_lowercase();
        }
    }
    "unknown".to_string()
}

pub async fn fetch_all_uids(
    email: &str,
    password: &str,
    folder: &str,
) -> Result<Vec<u32>, AppError> {
    let mut session = connect_imap(email, password, folder).await?;

    let uids = session
        .uid_search("ALL")
        .await
        .map_err(|e| AppError::Imap(e.to_string()))?;

    if let Err(e) = session.logout().await {
        tracing::warn!("Failed to logout after fetching UIDs: {}", e);
    }

    let mut uid_vec: Vec<u32> = uids.into_iter().collect();
    uid_vec.sort();
    Ok(uid_vec)
}

async fn scan_batch(
    email: &str,
    password: &str,
    folder: &str,
    uid_batch: &[u32],
) -> Result<Vec<String>, AppError> {
    if uid_batch.is_empty() {
        return Ok(Vec::new());
    }

    let mut session = connect_imap(email, password, folder).await?;

    let uid_str = uid_batch
        .iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let fetches = session
        .uid_fetch(&uid_str, "BODY.PEEK[HEADER.FIELDS (FROM)]")
        .await
        .map_err(|e| AppError::Imap(e.to_string()))?;

    let mut senders = Vec::new();
    use futures::StreamExt;
    let fetches: Vec<_> = fetches.collect::<Vec<_>>().await;
    for fetch_result in fetches {
        match fetch_result {
            Ok(fetch) => {
                if let Some(body) = fetch.header() {
                    let sender = parse_sender(body);
                    if sender != "unknown" {
                        senders.push(sender);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Fetch error in batch: {}", e);
            }
        }
    }

    if let Err(e) = session.logout().await {
        tracing::warn!("Failed to logout after scan batch: {}", e);
    }
    Ok(senders)
}

pub async fn run_scan<F>(
    email: &str,
    password: &str,
    folder: &str,
    uids: Vec<u32>,
    progress_cb: F,
) -> Result<Vec<SenderInfo>, AppError>
where
    F: Fn(f32, String) + Send + Sync + 'static,
{
    let total = uids.len();
    if total == 0 {
        return Ok(Vec::new());
    }

    let chunk_size = (total / MAX_CONCURRENT).max(1);
    let chunks: Vec<Vec<u32>> = uids.chunks(chunk_size).map(|c| c.to_vec()).collect();
    let num_chunks = chunks.len();

    let sender_map: Arc<Mutex<HashMap<String, usize>>> = Arc::new(Mutex::new(HashMap::new()));
    let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let progress_cb = Arc::new(progress_cb);

    let mut handles = Vec::new();

    let email: Arc<str> = Arc::from(email);
    let password: Arc<str> = Arc::from(password);
    let folder: Arc<str> = Arc::from(folder);

    for chunk in chunks {
        let email = email.clone();
        let password = password.clone();
        let folder = folder.clone();
        let map = sender_map.clone();
        let completed = completed.clone();
        let cb = progress_cb.clone();

        let handle = tokio::spawn(async move {
            match scan_batch(&email, &password, &folder, &chunk).await {
                Ok(senders) => {
                    let mut m = map.lock().await;
                    for s in senders {
                        *m.entry(s).or_insert(0) += 1;
                    }
                }
                Err(e) => {
                    tracing::error!("Batch scan error: {}", e);
                }
            }

            let done = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let progress = 0.05 + 0.95 * (done as f32 / num_chunks as f32);
            cb(progress, format!("Scanned batch {}/{}", done, num_chunks));
        });

        handles.push(handle);
    }

    for handle in handles {
        if let Err(e) = handle.await {
            tracing::error!("Scan task panicked: {}", e);
        }
    }

    let map = sender_map.lock().await;
    let mut senders: Vec<SenderInfo> = map
        .iter()
        .map(|(email, count)| SenderInfo {
            email: email.clone(),
            count: *count,
        })
        .collect();

    senders.sort_by(|a, b| b.count.cmp(&a.count));
    Ok(senders)
}
