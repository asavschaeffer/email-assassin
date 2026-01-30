use crate::imap::{deleter, scanner};
use crate::state::{DeleteMode, SenderInfo};
use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc as tokio_mpsc;
use tracing::{error, info};

#[derive(Debug)]
pub enum UiCommand {
    StartScan {
        email: String,
        password: String,
        folder: String,
        scan_depth: u32,
    },
    StartDelete {
        email: String,
        password: String,
        folder: String,
        senders: Vec<String>,
        mode: DeleteMode,
    },
}

#[derive(Debug)]
pub enum BackgroundEvent {
    ScanProgress {
        progress: f32,
        status: String,
    },
    ScanComplete {
        senders: Vec<SenderInfo>,
        total_emails: usize,
    },
    ScanError(String),
    DeleteProgress {
        progress: f32,
        status: String,
    },
    DeleteComplete {
        removed_senders: Vec<String>,
        total_removed: usize,
    },
    DeleteError(String),
}

pub struct BridgeChannels {
    pub cmd_tx: tokio_mpsc::UnboundedSender<UiCommand>,
    pub event_rx: std_mpsc::Receiver<BackgroundEvent>,
}

pub fn setup_bridge(ctx: egui::Context) -> BridgeChannels {
    let (cmd_tx, cmd_rx) = tokio_mpsc::unbounded_channel::<UiCommand>();
    let (event_tx, event_rx) = std_mpsc::channel::<BackgroundEvent>();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(background_loop(cmd_rx, event_tx, ctx));
    });

    BridgeChannels { cmd_tx, event_rx }
}

async fn background_loop(
    mut cmd_rx: tokio_mpsc::UnboundedReceiver<UiCommand>,
    event_tx: std_mpsc::Sender<BackgroundEvent>,
    ctx: egui::Context,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            UiCommand::StartScan {
                email,
                password,
                folder,
                scan_depth,
            } => {
                let tx = event_tx.clone();
                let ctx2 = ctx.clone();
                tokio::spawn(async move {
                    handle_scan(email, password, folder, scan_depth, tx, ctx2).await;
                });
            }
            UiCommand::StartDelete {
                email,
                password,
                folder,
                senders,
                mode,
            } => {
                let tx = event_tx.clone();
                let ctx2 = ctx.clone();
                tokio::spawn(async move {
                    handle_delete(email, password, folder, senders, mode, tx, ctx2).await;
                });
            }
        }
    }
}

async fn handle_scan(
    email: String,
    password: String,
    folder: String,
    scan_depth: u32,
    tx: std_mpsc::Sender<BackgroundEvent>,
    ctx: egui::Context,
) {
    let send = |evt: BackgroundEvent| {
        if let Err(e) = tx.send(evt) {
            tracing::warn!("Failed to send scan event to UI: {}", e);
        }
        ctx.request_repaint();
    };

    send(BackgroundEvent::ScanProgress {
        progress: 0.0,
        status: "Fetching message IDs...".to_string(),
    });

    let all_uids = match scanner::fetch_all_uids(&email, &password, &folder).await {
        Ok(uids) => uids,
        Err(e) => {
            send(BackgroundEvent::ScanError(e.to_string()));
            return;
        }
    };

    let total_emails = all_uids.len();
    let uids_to_scan = if scan_depth > 0 && (scan_depth as usize) < total_emails {
        all_uids[total_emails - scan_depth as usize..].to_vec()
    } else {
        all_uids
    };

    send(BackgroundEvent::ScanProgress {
        progress: 0.05,
        status: format!(
            "Found {} emails, scanning {}...",
            total_emails,
            uids_to_scan.len()
        ),
    });

    let progress_cb = {
        let tx2 = tx.clone();
        let ctx2 = ctx.clone();
        move |progress: f32, status: String| {
            if let Err(e) = tx2.send(BackgroundEvent::ScanProgress { progress, status }) {
                tracing::warn!("Failed to send scan progress to UI: {}", e);
            }
            ctx2.request_repaint();
        }
    };

    match scanner::run_scan(&email, &password, &folder, uids_to_scan, progress_cb).await {
        Ok(senders) => {
            send(BackgroundEvent::ScanComplete {
                senders,
                total_emails,
            });
        }
        Err(e) => {
            send(BackgroundEvent::ScanError(e.to_string()));
        }
    }
}

async fn handle_delete(
    email: String,
    password: String,
    folder: String,
    senders: Vec<String>,
    mode: DeleteMode,
    tx: std_mpsc::Sender<BackgroundEvent>,
    ctx: egui::Context,
) {
    let send = |evt: BackgroundEvent| {
        if let Err(e) = tx.send(evt) {
            tracing::warn!("Failed to send delete event to UI: {}", e);
        }
        ctx.request_repaint();
    };

    let total = senders.len();
    let mut total_removed = 0usize;
    let mut removed_senders = Vec::new();
    let use_trash = mode == DeleteMode::Trash;

    for (i, sender) in senders.iter().enumerate() {
        send(BackgroundEvent::DeleteProgress {
            progress: i as f32 / total as f32,
            status: format!("Purging {}...", sender),
        });

        match deleter::nuke_sender(&email, &password, &folder, sender, use_trash).await {
            Ok(count) => {
                total_removed += count;
                removed_senders.push(sender.clone());
                info!("Removed {} emails from {}", count, sender);
            }
            Err(e) => {
                error!("Failed to delete emails from {}: {}", sender, e);
                send(BackgroundEvent::DeleteError(format!(
                    "Failed to purge {}: {}",
                    sender, e
                )));
            }
        }

        send(BackgroundEvent::DeleteProgress {
            progress: (i + 1) as f32 / total as f32,
            status: format!("Completed {}/{}", i + 1, total),
        });
    }

    send(BackgroundEvent::DeleteComplete {
        removed_senders,
        total_removed,
    });
}
