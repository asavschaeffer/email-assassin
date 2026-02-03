use crate::bridge::UiCommand;
use crate::state::{AppPhase, AppState, DeleteMode};
use egui::Ui;
use tokio::sync::mpsc::UnboundedSender;

pub fn draw_sidebar(ui: &mut Ui, state: &mut AppState, cmd_tx: &UnboundedSender<UiCommand>) {
    let busy = matches!(
        state.phase,
        AppPhase::Scanning | AppPhase::Deleting | AppPhase::Connecting
    );

    // ── Stage 1: Credentials ──────────────────────────────────────

    ui.heading("Credentials");
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        for (icon, tooltip, domain) in [
            ("G", "Gmail", "@gmail.com"),
            ("O", "Outlook", "@outlook.com"),
            ("Y!", "Yahoo", "@yahoo.com"),
            ("\u{2601}", "iCloud", "@icloud.com"),
        ] {
            let btn = ui.add_enabled(!busy, egui::Button::new(icon).small());
            if btn.clicked() {
                if let Some(at) = state.email.find('@') {
                    state.email.replace_range(at.., domain);
                } else {
                    state.email.push_str(domain);
                }
                state.available_folders.clear();
            }
            btn.on_hover_text(tooltip);
        }
    });

    ui.add_space(4.0);

    ui.label("Email");
    let email_resp = ui.add_enabled(
        !busy,
        egui::TextEdit::singleline(&mut state.email).hint_text("you@gmail.com"),
    );

    ui.add_space(4.0);
    ui.label("App Password");
    let pass_resp = ui.add_enabled(
        !busy,
        egui::TextEdit::singleline(&mut state.password)
            .password(true)
            .hint_text("app password"),
    );

    // Credential change invalidation: collapse back to Stage 1
    if email_resp.changed() || pass_resp.changed() {
        state.available_folders.clear();
    }

    ui.add_space(8.0);

    let can_connect = !busy
        && state.email.contains('@')
        && !state.password.is_empty();

    if state.phase == AppPhase::Connecting {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Connecting...");
        });
    } else if ui
        .add_enabled(can_connect, egui::Button::new("Connect"))
        .clicked()
    {
        state.phase = AppPhase::Connecting;
        state.error_message = None;
        state.available_folders.clear();
        let _ = cmd_tx.send(UiCommand::FetchFolders {
            email: state.email.clone(),
            password: state.password.clone(),
        });
    }

    // ── Stage 2: Folder & Scan config (only after successful connect) ──

    if !state.available_folders.is_empty() {
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        ui.label("Folder");
        let selected = state.folder.clone();
        egui::ComboBox::from_id_salt("folder_selector")
            .selected_text(&selected)
            .width(ui.available_width() - 8.0)
            .show_ui(ui, |ui| {
                for folder in &state.available_folders {
                    ui.selectable_value(&mut state.folder, folder.clone(), folder);
                }
            });

        ui.add_space(8.0);

        ui.label("Scan Depth (0 = all)");
        ui.add_enabled(
            !busy,
            egui::Slider::new(&mut state.scan_depth, 0..=50000),
        );

        ui.add_space(8.0);

        let can_scan = !busy;
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
}
