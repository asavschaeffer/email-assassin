use crate::state::SenderInfo;
use egui::{Color32, Pos2, Sense, Shape, Stroke, Vec2};
use std::f32::consts::TAU;

const PALETTE: &[Color32] = &[
    Color32::from_rgb(239, 71, 111),
    Color32::from_rgb(255, 209, 102),
    Color32::from_rgb(6, 214, 160),
    Color32::from_rgb(17, 138, 178),
    Color32::from_rgb(7, 59, 76),
    Color32::from_rgb(230, 57, 70),
    Color32::from_rgb(241, 250, 238),
    Color32::from_rgb(168, 218, 220),
    Color32::from_rgb(69, 123, 157),
    Color32::from_rgb(29, 53, 87),
    Color32::from_rgb(255, 190, 11),
    Color32::from_rgb(251, 86, 7),
    Color32::from_rgb(255, 0, 110),
    Color32::from_rgb(131, 56, 236),
    Color32::from_rgb(58, 134, 255),
    Color32::from_rgb(181, 23, 158),
    Color32::from_rgb(254, 228, 64),
    Color32::from_rgb(0, 187, 249),
    Color32::from_rgb(114, 9, 183),
    Color32::from_rgb(247, 127, 0),
];

pub fn draw_donut(ui: &mut egui::Ui, senders: &[SenderInfo], max_slices: usize) {
    let available = ui.available_size();
    let size = available.x.min(available.y).min(300.0);
    let (response, painter) = ui.allocate_painter(Vec2::splat(size), Sense::hover());
    let rect = response.rect;
    let center = rect.center();
    let outer_r = size * 0.45;
    let inner_r = size * 0.25;

    let top_senders: Vec<&SenderInfo> = senders.iter().take(max_slices).collect();
    let total: usize = top_senders.iter().map(|s| s.count).sum();
    if total == 0 {
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            "No data",
            egui::FontId::proportional(14.0),
            Color32::GRAY,
        );
        return;
    }

    let mouse_pos = response.hover_pos();
    let mut start_angle: f32 = -TAU / 4.0; // Start from top
    let mut hovered_sender: Option<(&str, usize)> = None;

    for (i, sender) in top_senders.iter().enumerate() {
        let fraction = sender.count as f32 / total as f32;
        let sweep = fraction * TAU;
        let color = PALETTE[i % PALETTE.len()];

        // Build arc polygon
        let segments = (sweep / 0.05).max(2.0) as usize;
        let mut points = Vec::with_capacity(segments * 2 + 2);

        // Outer arc
        for j in 0..=segments {
            let angle = start_angle + sweep * (j as f32 / segments as f32);
            points.push(Pos2::new(
                center.x + outer_r * angle.cos(),
                center.y + outer_r * angle.sin(),
            ));
        }
        // Inner arc (reversed)
        for j in (0..=segments).rev() {
            let angle = start_angle + sweep * (j as f32 / segments as f32);
            points.push(Pos2::new(
                center.x + inner_r * angle.cos(),
                center.y + inner_r * angle.sin(),
            ));
        }

        // Hit test for hover
        let mut is_hovered = false;
        if let Some(mp) = mouse_pos {
            let dx = mp.x - center.x;
            let dy = mp.y - center.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist >= inner_r && dist <= outer_r {
                let mut angle = dy.atan2(dx);
                // Normalize to same range as start_angle
                if angle < start_angle {
                    angle += TAU;
                }
                let end_angle = start_angle + sweep;
                if angle >= start_angle && angle <= end_angle
                    || angle + TAU >= start_angle && angle + TAU <= end_angle
                {
                    is_hovered = true;
                    hovered_sender = Some((&sender.email, sender.count));
                }
            }
        }

        let fill = if is_hovered {
            Color32::from_rgba_premultiplied(
                color.r().saturating_add(40),
                color.g().saturating_add(40),
                color.b().saturating_add(40),
                255,
            )
        } else {
            color
        };

        painter.add(Shape::convex_polygon(
            points,
            fill,
            Stroke::new(1.0, Color32::from_gray(30)),
        ));

        start_angle += sweep;
    }

    // Center label — truncate to fit the donut hole at 11pt proportional font.
    // 25 chars is the display threshold; 22 + "..." keeps it within bounds.
    if let Some((sender, count)) = hovered_sender {
        let truncated = if sender.chars().count() > 25 {
            format!("{}...", sender.chars().take(22).collect::<String>())
        } else {
            sender.to_string()
        };
        painter.text(
            center + Vec2::new(0.0, -8.0),
            egui::Align2::CENTER_CENTER,
            truncated,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
        painter.text(
            center + Vec2::new(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            format!("{count} emails"),
            egui::FontId::proportional(11.0),
            Color32::LIGHT_GRAY,
        );
    }
}
