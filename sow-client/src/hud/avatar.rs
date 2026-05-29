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
