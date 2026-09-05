use egui::Rect;
use super::layout;

/// Native profile data is a presentation snapshot. Network requests stay in sow-client.
#[derive(Default)]
pub struct NativeProfileState {
    pub public_id: Option<String>,
    pub view: Option<sow_data::profile::PublicProfileView>,
    pub history: Vec<sow_data::profile::PublicMatchSummary>,
    pub ratings: Vec<sow_data::profile::PublicRatingView>,
    pub search_results: Vec<sow_data::profile::PublicProfileSummary>,
    pub search_query: String,
    pub history_cursor: usize,
    pub history_has_next: bool,
    pub match_detail: Option<sow_data::profile::PublicMatchDetail>,
    pub ratings_loaded: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub active_tab: NativeProfileTab,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeProfileTab {
    #[default]
    Overview,
    Leaders,
    History,
    Ranked,
}

#[path = "profile_view.rs"]
mod profile_view;
pub use profile_view::draw_native;

const MAIN_MENU_AVATAR_RECT_KEY: &str = "main_menu_avatar_rect";

fn main_menu_avatar_rect_id() -> egui::Id {
    egui::Id::new(MAIN_MENU_AVATAR_RECT_KEY)
}

/// Screen rect of the main-menu leader avatar button (same frame the picker opens from).
pub fn main_menu_avatar_button_rect(ctx: &egui::Context) -> Rect {
    let id = main_menu_avatar_rect_id();
    if let Some(rect) = ctx.data(|d| d.get_temp::<Rect>(id)) {
        return rect;
    }
    // Fallback mirrors the persistent shell header when the header has not
    // painted its avatar rect yet (for example, before the first frame).
    let screen = ctx.content_rect();
    let metrics = layout::main_menu_metrics(ctx);
    let inset = if metrics.is_phone() { 8.0 } else { 10.0 };
    Rect::from_min_size(
        egui::pos2(
            screen.min.x + inset,
            screen.min.y + 7.0,
        ),
        egui::vec2(44.0, 44.0),
    )
}
