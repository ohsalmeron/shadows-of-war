use egui::Context;

/// The three layout classes used by every native menu screen.
///
/// The square 848×848 window intentionally lands in `Compact`: width alone is
/// not enough room for the desktop command bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewportClass {
    Wide,
    Compact,
    Phone,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MainMenuMetrics {
    pub class: ViewportClass,
    pub scale: f32,
    pub outer_pad: f32,
    pub gap: f32,
    pub touch_min: f32,
}

impl MainMenuMetrics {
    pub(crate) fn is_phone(self) -> bool {
        self.class == ViewportClass::Phone
    }

    pub(crate) fn is_compact(self) -> bool {
        self.class != ViewportClass::Wide
    }

    pub(crate) fn columns(self) -> usize {
        match self.class {
            ViewportClass::Wide => 3,
            ViewportClass::Compact => 2,
            ViewportClass::Phone => 1,
        }
    }

}

pub(crate) fn main_menu_metrics(ctx: &Context) -> MainMenuMetrics {
    let rect = ctx.content_rect();
    let width = rect.width().max(1.0);
    let height = rect.height().max(1.0);
    let class = viewport_class(width, height);
    let scale = ((width / 768.0).min(height / 720.0)).clamp(0.72, 1.0);
    let (outer_pad, gap) = match class {
        ViewportClass::Wide => (24.0, 16.0),
        ViewportClass::Compact => (16.0, 12.0),
        ViewportClass::Phone => (12.0, 8.0),
    };
    MainMenuMetrics {
        class,
        scale,
        outer_pad,
        gap,
        touch_min: 44.0,
    }
}

pub(crate) fn viewport_class(width: f32, height: f32) -> ViewportClass {
    if width < height {
        ViewportClass::Phone
    } else if width >= 1024.0 && width >= height * 1.25 {
        ViewportClass::Wide
    } else {
        ViewportClass::Compact
    }
}

pub(crate) fn menu_layout_chrome(
    ctx: &egui::Context,
    panel_h: f32,
    available_w: f32,
    compact: bool,
) -> (f32, f32, f32, f32) {
    let metrics = main_menu_metrics(ctx);
    let compact = compact || metrics.is_compact();
    let mut section_gap = metrics.gap;
    let mut action_min_h = metrics.touch_min;
    let mut profile_height = if metrics.is_phone() { 52.0 } else { 56.0 };

    let mut lobby_h = crate::ui::map_texture::thumbnail_square_side(available_w, compact);
    if metrics.is_phone() {
        lobby_h = (lobby_h * 0.55).clamp(110.0, 160.0);
    }

    let needed = if metrics.is_phone() {
        profile_height + section_gap * 2.0 + action_min_h * 2.0 + lobby_h
    } else {
        profile_height + section_gap * 2.0 + action_min_h.max(lobby_h)
    };
    let shrink = sow_ui_kit::theme::fit_scale(needed, panel_h);
    if shrink < 1.0 {
        section_gap *= shrink;
        action_min_h = (action_min_h * shrink).max(metrics.touch_min);
        profile_height *= shrink;
        lobby_h *= shrink;
    }
    (section_gap, action_min_h, profile_height, lobby_h)
}

#[cfg(test)]
mod tests {
    use super::{viewport_class, ViewportClass};

    #[test]
    fn menu_viewport_classes_keep_square_windows_compact() {
        assert_eq!(viewport_class(360.0, 800.0), ViewportClass::Phone);
        assert_eq!(viewport_class(390.0, 844.0), ViewportClass::Phone);
        assert_eq!(viewport_class(540.0, 720.0), ViewportClass::Phone);
        assert_eq!(viewport_class(800.0, 600.0), ViewportClass::Compact);
        assert_eq!(viewport_class(848.0, 848.0), ViewportClass::Compact);
        assert_eq!(viewport_class(1280.0, 720.0), ViewportClass::Wide);
        assert_eq!(viewport_class(1920.0, 1080.0), ViewportClass::Wide);
    }
}
