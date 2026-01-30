use crate::bridge::UiCommand;
use crate::state::{AppPhase, AppState};
use crate::ui::donut;
use egui::Ui;
use tokio::sync::mpsc::UnboundedSender;

pub fn draw_dashboard(ui: &mut Ui, state: &mut AppState, cmd_tx: &UnboundedSender<UiCommand>) {
    let busy = state.phase == AppPhase::Scanning || state.phase == AppPhase::Deleting;

    // Error display
    if let Some(err) = &state.error_message {
        ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
        ui.add_space(4.0);
    }

    // Progress display
    match state.phase {
        AppPhase::Scanning => {
            ui.heading("Scanning...");
            ui.add(egui::ProgressBar::new(state.scan_progress).text(&state.scan_status));
            ui.add_space(8.0);
        }
        AppPhase::Deleting => {
            ui.heading("Deleting...");
            ui.add(egui::ProgressBar::new(state.delete_progress).text(&state.delete_status));
            ui.add_space(8.0);
        }
        _ => {}
    }

    if state.senders.is_empty() && state.phase == AppPhase::Idle {
        ui.centered_and_justified(|ui| {
            ui.label("Enter credentials and click Start Scan to begin.");
        });
        return;
    }

    if state.senders.is_empty() {
        return;
    }

    // Metrics row
    ui.horizontal(|ui| {
        let frame = egui::Frame::default()
            .inner_margin(8.0)
            .corner_radius(4.0)
            .fill(ui.visuals().faint_bg_color);

        frame.show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label("Emails Scanned");
                ui.heading(state.total_scanned().to_string());
            });
        });

        frame.show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label("Unique Senders");
                ui.heading(state.unique_senders().to_string());
            });
        });

        frame.show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label("Total in Folder");
                ui.heading(state.total_emails.to_string());
            });
        });
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // Two-column layout: donut + kill list
    ui.columns(2, |columns| {
        // Left: Donut chart
        columns[0].heading("Inbox Composition");
        columns[0].add_space(4.0);
        donut::draw_donut(&mut columns[0], &state.senders, 20);

        // Right: Kill list
        columns[1].heading("Kill List");
        columns[1].add_space(4.0);
        draw_kill_list(&mut columns[1], state, cmd_tx, busy);
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // Raw data table
    draw_raw_table(ui, state);
}

fn draw_kill_list(
    ui: &mut Ui,
    state: &mut AppState,
    cmd_tx: &UnboundedSender<UiCommand>,
    busy: bool,
) {
    let top_senders: Vec<(String, usize)> = state
        .senders
        .iter()
        .take(100)
        .map(|s| (s.email.clone(), s.count))
        .collect();

    egui::ScrollArea::vertical()
        .max_height(250.0)
        .show(ui, |ui| {
            for (email, count) in &top_senders {
                let checked = state.sender_selected.entry(email.clone()).or_insert(false);
                ui.horizontal(|ui| {
                    ui.checkbox(checked, "");
                    ui.label(format!("{} ({})", email, count));
                });
            }
        });

    let selected_count = state.selected_email_count();
    if selected_count > 0 {
        ui.add_space(4.0);
        ui.colored_label(
            egui::Color32::YELLOW,
            format!("~{} emails selected for removal", selected_count),
        );

        if ui
            .add_enabled(!busy, egui::Button::new("EXECUTE"))
            .clicked()
        {
            let selected: Vec<String> = state
                .senders
                .iter()
                .filter(|s| state.sender_selected.get(&s.email).copied().unwrap_or(false))
                .map(|s| s.email.clone())
                .collect();

            state.phase = AppPhase::Deleting;
            state.delete_progress = 0.0;
            state.delete_status = "Starting deletion...".to_string();
            state.error_message = None;

            let _ = cmd_tx.send(UiCommand::StartDelete {
                email: state.email.clone(),
                password: state.password.clone(),
                folder: state.folder.clone(),
                senders: selected,
                mode: state.delete_mode.clone(),
            });
        }
    }
}

fn draw_raw_table(ui: &mut Ui, state: &AppState) {
    ui.collapsing("Raw Data", |ui| {
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .column(egui_extras::Column::remainder().at_least(200.0))
            .column(egui_extras::Column::initial(80.0))
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Sender");
                });
                header.col(|ui| {
                    ui.strong("Count");
                });
            })
            .body(|body| {
                body.rows(18.0, state.senders.len(), |mut row| {
                    let idx = row.index();
                    if let Some(sender) = state.senders.get(idx) {
                        row.col(|ui| {
                            ui.label(&sender.email);
                        });
                        row.col(|ui| {
                            ui.label(sender.count.to_string());
                        });
                    }
                });
            });
    });
}
