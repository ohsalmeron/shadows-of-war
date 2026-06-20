/// Paints a circular avatar with a decorative ring frame.
/// For textured avatars, clips to a circle via a triangle-fan mesh.
/// For solid-color avatars (nations), fills a circle.
pub fn paint_circular_avatar(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    texture: Option<egui::TextureId>,
    fill_color: egui::Color32,
    frame_color: egui::Color32,
) {
    const SEGMENTS: usize = 32;

    if let Some(tex_id) = texture {
        let mut mesh = egui::Mesh::with_texture(tex_id);
        mesh.vertices.push(egui::epaint::Vertex {
            pos: center,
            uv: egui::pos2(0.5, 0.5),
            color: egui::Color32::WHITE,
        });
        for i in 0..=SEGMENTS {
            let angle = (i as f32 / SEGMENTS as f32) * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            mesh.vertices.push(egui::epaint::Vertex {
                pos: egui::pos2(center.x + cos * radius, center.y + sin * radius),
                uv: egui::pos2(0.5 + cos * 0.5, 0.5 + sin * 0.5),
                color: egui::Color32::WHITE,
            });
        }
        for i in 1..=SEGMENTS {
            mesh.indices.push(0);
            mesh.indices.push(i as u32);
            mesh.indices.push(i as u32 + 1);
        }
        painter.add(egui::Shape::mesh(mesh));
    } else {
        painter.circle_filled(center, radius, fill_color);
    }

    let border = (radius * 0.12).max(1.0);
    painter.circle_stroke(
        center,
        radius + border * 0.3,
        egui::Stroke::new(border, egui::Color32::from_black_alpha(160)),
    );
    painter.circle_stroke(center, radius, egui::Stroke::new(border * 0.8, frame_color));
    painter.circle_stroke(
        center,
        radius - border * 0.15,
        egui::Stroke::new(border * 0.35, egui::Color32::from_white_alpha(80)),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn draw_player_avatar(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    player_id: u16,
    player_name: &str,
    player_type: sow_core::player::PlayerType,
    player_color: [f32; 3],
    leader: &sow_core::player::Leader,
    asset_loader: &sow_ui::ui::asset_loader::AssetLoader,
) {
    let vibrant_color = crate::hud::nameplate::ensure_readable_nameplate_color(player_color);

    match player_type {
        sow_core::player::PlayerType::Nation => {
            paint_circular_avatar(painter, center, radius, None, vibrant_color, vibrant_color);
        }
        sow_core::player::PlayerType::Bot => {
            paint_circular_avatar(painter, center, radius, None, vibrant_color, vibrant_color);
            let animal = sow_core::player::tribe_animal(player_id, player_name);
            let emoji_size = radius * 2.0 * 0.7;
            let emoji_rect =
                egui::Rect::from_center_size(center, egui::vec2(emoji_size, emoji_size));
            if !sow_ui_kit::widgets::try_paint_emoji(
                painter,
                animal,
                emoji_rect,
                egui::Color32::WHITE,
            ) {
                let emoji_galley = painter.layout_no_wrap(
                    animal.to_owned(),
                    egui::FontId::proportional(emoji_size),
                    egui::Color32::WHITE,
                );
                let emoji_pos = egui::pos2(
                    center.x - emoji_galley.size().x / 2.0,
                    center.y - emoji_galley.size().y / 2.0,
                );
                painter.galley(emoji_pos, emoji_galley, egui::Color32::WHITE);
            }
        }
        sow_core::player::PlayerType::Human => {
            let leader_rgb = leader.filler_rgb();
            let leader_color = egui::Color32::from_rgb(
                (leader_rgb[0] * 255.0).round() as u8,
                (leader_rgb[1] * 255.0).round() as u8,
                (leader_rgb[2] * 255.0).round() as u8,
            );
            let avatar_tex = asset_loader
                .avatars
                .get(leader)
                .or(asset_loader.avatar_fallback.as_ref());
            let tex_id = avatar_tex.map(|t| t.id());
            paint_circular_avatar(painter, center, radius, tex_id, leader_color, leader_color);
        }
    }
}
