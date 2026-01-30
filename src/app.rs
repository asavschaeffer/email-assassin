use crate::bridge::{BackgroundEvent, UiCommand};
use crate::state::{AppPhase, AppState};
use crate::ui::{dashboard, sidebar};
use tokio::sync::mpsc::UnboundedSender;

pub struct EmailAssassinApp {
    state: AppState,
    cmd_tx: UnboundedSender<UiCommand>,
    event_rx: std::sync::mpsc::Receiver<BackgroundEvent>,
}

impl EmailAssassinApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let bridge = crate::bridge::setup_bridge(cc.egui_ctx.clone());
        Self {
            state: AppState::default(),
            cmd_tx: bridge.cmd_tx,
            event_rx: bridge.event_rx,
        }
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                BackgroundEvent::ScanProgress { progress, status } => {
                    self.state.scan_progress = progress;
                    self.state.scan_status = status;
                }
                BackgroundEvent::ScanComplete {
                    senders,
                    total_emails,
                } => {
                    self.state.senders = senders;
                    self.state.total_emails = total_emails;
                    self.state.phase = AppPhase::ScanComplete;
                    self.state.scan_progress = 1.0;
                    self.state.scan_status = "Complete".to_string();
                }
                BackgroundEvent::ScanError(msg) => {
                    self.state.error_message = Some(msg);
                    self.state.phase = AppPhase::Idle;
                }
                BackgroundEvent::DeleteProgress { progress, status } => {
                    self.state.delete_progress = progress;
                    self.state.delete_status = status;
                }
                BackgroundEvent::DeleteComplete {
                    removed_senders,
                    total_removed,
                } => {
                    // Optimistic update: remove deleted senders
                    self.state
                        .senders
                        .retain(|s| !removed_senders.contains(&s.email));
                    for sender in &removed_senders {
                        self.state.sender_selected.remove(sender);
                    }
                    self.state.phase = AppPhase::ScanComplete;
                    self.state.delete_progress = 1.0;
                    self.state.delete_status =
                        format!("Removed {} emails", total_removed);
                }
                BackgroundEvent::DeleteError(msg) => {
                    self.state.error_message = Some(msg);
                    // Don't reset phase - partial failure is tolerated
                }
            }
            ctx.request_repaint();
        }
    }
}

impl eframe::App for EmailAssassinApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events(ctx);

        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    sidebar::draw_sidebar(ui, &mut self.state, &self.cmd_tx);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                dashboard::draw_dashboard(ui, &mut self.state, &self.cmd_tx);
            });
        });
    }
}
