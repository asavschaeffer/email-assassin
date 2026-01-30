use crate::error::AppError;
use crate::state::SenderInfo;
use futures::StreamExt;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::mpsc;

use super::connect_imap;

static FROM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)From:\s*(.*)").unwrap());
static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<([^>]+)>").unwrap());

/// Number of persistent IMAP connections used for parallel scanning.
/// Balances throughput against server-side connection limits (most
/// providers allow 10-15 simultaneous sessions).
const MAX_CONCURRENT: usize = 10;

/// Initial progress percentage reserved for the UID-fetch phase before
/// batch scanning begins.
const INITIAL_PROGRESS: f32 = 0.05;

fn parse_sender(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw);
    if let Some(m) = FROM_RE.captures(&text) {
        let raw_from = m.get(1).map_or("", |m| m.as_str().trim());
        if let Some(email_match) = EMAIL_RE.captures(raw_from) {
            return email_match
                .get(1).map_or_else(|| "unknown".to_string(), |m| m.as_str().to_lowercase());
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
        tracing::warn!(error = %e, "logout failed after UID fetch");
    }

    let mut uid_vec: Vec<u32> = uids.into_iter().collect();
    uid_vec.sort_unstable();
    Ok(uid_vec)
}

struct ScanWorker {
    email: String,
    password: String,
    folder: String,
    session: Option<async_imap::Session<async_native_tls::TlsStream<async_std::net::TcpStream>>>,
}

impl ScanWorker {
    fn new(email: String, password: String, folder: String) -> Self {
        Self {
            email,
            password,
            folder,
            session: None,
        }
    }

    async fn ensure_connected(&mut self) -> Result<(), AppError> {
        if self.session.is_some() {
            return Ok(());
        }
        let session = connect_imap(&self.email, &self.password, &self.folder).await?;
        self.session = Some(session);
        Ok(())
    }

    async fn scan_batch(&mut self, uids: &[u32]) -> Result<Vec<String>, AppError> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }

        self.ensure_connected().await?;
        let mut session = self.session.take().unwrap();

        let uid_str = uids
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");

        let fetches_result = session
            .uid_fetch(&uid_str, "BODY.PEEK[HEADER.FIELDS (FROM)]")
            .await;

        if let Err(e) = fetches_result {
            tracing::warn!(error = %e, "IMAP fetch failed, dropping session");
            return Err(AppError::Imap(e.to_string()));
        }

        let mut stream = fetches_result.unwrap();
        let mut senders = Vec::new();

        while let Some(fetch_result) = stream.next().await {
            if let Ok(fetch) = fetch_result {
                if let Some(body) = fetch.header() {
                    let sender = parse_sender(body);
                    if sender != "unknown" {
                        senders.push(sender);
                    }
                }
            }
        }

        // Explicitly drop the stream to release the borrow on session
        drop(stream);

        // Success — return the session to the worker for reuse
        self.session = Some(session);
        Ok(senders)
    }
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
    let chunks: Vec<Vec<u32>> = uids.chunks(chunk_size).map(<[u32]>::to_vec).collect();
    let num_chunks = chunks.len();

    let (job_tx, job_rx) = async_channel::bounded(num_chunks);
    let (result_tx, mut result_rx) = mpsc::channel(num_chunks + 10);

    for chunk in chunks {
        if let Err(e) = job_tx.send(chunk).await {
            tracing::error!(error = %e, "failed to enqueue scan job");
        }
    }
    job_tx.close();

    let mut handles = Vec::new();
    for worker_id in 0..MAX_CONCURRENT {
        let job_rx = job_rx.clone();
        let result_tx = result_tx.clone();
        let email = email.to_string();
        let password = password.to_string();
        let folder = folder.to_string();

        handles.push(tokio::spawn(async move {
            let mut worker = ScanWorker::new(email, password, folder);
            while let Ok(chunk) = job_rx.recv().await {
                match worker.scan_batch(&chunk).await {
                    Ok(senders) => {
                        if let Err(e) = result_tx.send(senders).await {
                            tracing::error!(worker = worker_id, error = %e, "failed to send scan result");
                        }
                    }
                    Err(e) => {
                        tracing::error!(worker = worker_id, error = %e, "batch scan failed");
                        // Send empty result to keep progress moving
                        if let Err(e) = result_tx.send(Vec::new()).await {
                            tracing::error!(worker = worker_id, error = %e, "failed to send error fallback");
                        }
                    }
                }
            }
            if let Some(mut session) = worker.session {
                if let Err(e) = session.logout().await {
                    tracing::warn!(worker = worker_id, error = %e, "logout failed after scan");
                }
            }
        }));
    }

    drop(result_tx);

    let mut sender_map = HashMap::new();
    let mut completed_batches = 0;

    while let Some(senders) = result_rx.recv().await {
        for s in senders {
            *sender_map.entry(s).or_insert(0) += 1;
        }

        completed_batches += 1;
        let progress = INITIAL_PROGRESS + (1.0 - INITIAL_PROGRESS) * (completed_batches as f32 / num_chunks as f32);
        progress_cb(progress, format!("Scanned batch {completed_batches}/{num_chunks}"));
    }

    let mut senders: Vec<SenderInfo> = sender_map
        .into_iter()
        .map(|(email, count)| SenderInfo { email, count })
        .collect();

    senders.sort_by(|a, b| b.count.cmp(&a.count));
    Ok(senders)
}
