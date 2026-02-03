use crate::bridge::UiCommand;
use crate::state::{AppPhase, AppState, DeleteMode};
use crate::ui::assets::AppAssets;
use egui::Ui;
use tokio::sync::mpsc::UnboundedSender;

pub fn draw_sidebar(
    ui: &mut Ui,
    state: &mut AppState,
    assets: &AppAssets,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    let busy = matches!(
        state.phase,
        AppPhase::Scanning | AppPhase::Deleting | AppPhase::Connecting
    );

    // ── Stage 1: Credentials ──────────────────────────────────────

    ui.heading("Credentials");
    ui.add_space(4.0);

    // ── Provider logo buttons ──
    ui.horizontal_wrapped(|ui| {
        let providers = [
            (&assets.icon_gmail, "Gmail", "@gmail.com"),
            (&assets.icon_outlook, "Outlook", "@outlook.com"),
            (&assets.icon_yahoo, "Yahoo", "@yahoo.com"),
            (&assets.icon_icloud, "iCloud", "@icloud.com"),
        ];

        let selected = state.email_domain.to_lowercase();

        for (texture, tooltip, prov_domain) in providers {
            let is_selected = selected == *prov_domain;

            let bg_color = if is_selected {
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 40)
            } else {
                egui::Color32::TRANSPARENT
            };

            let img_btn = egui::ImageButton::new(
                egui::Image::new(texture)
                    .fit_to_exact_size(egui::vec2(24.0, 24.0))
                    .bg_fill(bg_color),
            )
            .frame(false)
            .corner_radius(4.0);

            let btn_response = ui.add_enabled(!busy, img_btn).on_hover_text(tooltip);
            if btn_response.clicked() {
                state.email_domain = prov_domain.to_string();
                state.available_folders.clear();
            }
        }
    });

    ui.add_space(4.0);

    // ── Username field with domain suffix overlay ──
    ui.label("Email");
    let email_resp = ui.add_enabled(
        !busy,
        egui::TextEdit::singleline(&mut state.email_user).hint_text("username"),
    );

    // If user pasted a full email, split off the domain
    if let Some(at) = state.email_user.find('@') {
        let domain = state.email_user[at..].to_owned();
        state.email_user.truncate(at);
        if !domain.eq_ignore_ascii_case(&state.email_domain) {
            state.email_domain = domain;
            state.available_folders.clear();
        }
    }

    // Paint domain suffix right-aligned inside the text field
    if !state.email_domain.is_empty() {
        let rect = email_resp.rect;
        let font_id = egui::TextStyle::Body.resolve(ui.style());
        let color = ui.visuals().weak_text_color();
        let padding = ui.spacing().button_padding.x;
        ui.painter().text(
            egui::pos2(rect.right() - padding, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            &state.email_domain,
            font_id,
            color,
        );
    }

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

    let has_username = !state.email_user.is_empty();
    let has_domain = !state.email_domain.is_empty();
    let can_connect = !busy && has_username && has_domain && !state.password.is_empty();

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
            email: state.email(),
            password: state.password.clone(),
        });
    }

    // ── Stage 2: Folder & Scan config (only after successful connect) ──

    if !state.available_folders.is_empty() {
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        ui.label("Folder");
        egui::ComboBox::from_id_salt("folder_selector")
            .selected_text(&state.folder)
            .width(ui.available_width() - 8.0)
            .show_ui(ui, |ui| {
                for folder in &state.available_folders {
                    ui.selectable_value(&mut state.folder, folder.clone(), folder);
                }
            });

        ui.add_space(8.0);

        ui.label("Scan Depth (0 = all)");
        ui.add_enabled(!busy, egui::Slider::new(&mut state.scan_depth, 0..=50000));

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
                email: state.email(),
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
