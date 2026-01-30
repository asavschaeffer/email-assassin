use crate::bridge::UiCommand;
use crate::state::{AppPhase, AppState, DeleteMode};
use egui::Ui;
use tokio::sync::mpsc::UnboundedSender;

pub fn draw_sidebar(ui: &mut Ui, state: &mut AppState, cmd_tx: &UnboundedSender<UiCommand>) {
    let busy = state.phase == AppPhase::Scanning || state.phase == AppPhase::Deleting;

    ui.heading("Credentials");
    ui.add_space(4.0);

    ui.label("Email");
    ui.add_enabled(!busy, egui::TextEdit::singleline(&mut state.email).hint_text("you@gmail.com"));

    ui.add_space(4.0);
    ui.label("App Password");
    ui.add_enabled(
        !busy,
        egui::TextEdit::singleline(&mut state.password)
            .password(true)
            .hint_text("app password"),
    );

    ui.add_space(4.0);
    ui.label("Folder");
    ui.add_enabled(!busy, egui::TextEdit::singleline(&mut state.folder).hint_text("INBOX"));

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    ui.label("Scan Depth (0 = all)");
    ui.add_enabled(
        !busy,
        egui::Slider::new(&mut state.scan_depth, 0..=50000),
    );

    ui.add_space(8.0);

    let can_scan = !busy && !state.email.is_empty() && state.email.contains('@') && !state.password.is_empty();
    if ui
        .add_enabled(can_scan, egui::Button::new("Start Scan"))
        .clicked()
    {
        state.phase = AppPhase::Scanning;
        state.scan_progress = 0.0;
        state.scan_status = "Starting...".to_string();
        state.error_message = None;
        state.senders.clear();
        state.sender_selected.clear();

        let _ = cmd_tx.send(UiCommand::StartScan {
            email: state.email.clone(),
            password: state.password.clone(),
            folder: state.folder.clone(),
            scan_depth: state.scan_depth,
        });
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    ui.label("Delete Mode");
    ui.radio_value(&mut state.delete_mode, DeleteMode::Trash, "Move to Trash");
    ui.radio_value(
        &mut state.delete_mode,
        DeleteMode::Permanent,
        "Permanently Delete",
    );
}
