use egui::Color32;
use sow_i18n::Language;

const FADEIN_DURATION: f64 = 0.25;
const FADEOUT_DURATION: f64 = 0.25;
/// Minimum hold time BETWEEN fade-in and fade-out (loading can proceed during this).
/// Total visible time = FADEIN + MIN_HOLD + FADEOUT = 0.25 + 1.0 + 0.25 = 1.5s
const MIN_HOLD_DURATION: f64 = 1.0;

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Debug, Clone, PartialEq)]
pub enum SplashJob {
    Boot,
    EnterGame,
    ExitGame,
}

pub struct SplashState {
    pub job: SplashJob,
    pub status_text: String,
    pub status_override: Option<String>,
    pub progress: f32,
    pub frames_drawn: u32,
    pub thumbnail: Option<egui::TextureHandle>,
    pub gpu_load_step: u8,
    pub done: bool,
    pub start_time: Option<f64>,
    pub fadeout_start: Option<f64>,
    pub opacity: f32,
    pub target_phase: Option<crate::app::ClientPhase>,
    pub visual_progress: f32,
    pub last_update_time: Option<f64>,
    pub random_speed: f32,
}

impl Default for SplashState {
    fn default() -> Self {
        Self {
            job: SplashJob::Boot,
            status_text: String::new(),
            status_override: None,
            progress: 0.0,
            frames_drawn: 0,
            thumbnail: None,
            gpu_load_step: 0,
            done: false,
            start_time: None,
            fadeout_start: None,
            opacity: 1.0,
            target_phase: None,
            visual_progress: 0.0,
            last_update_time: None,
            random_speed: 0.0,
        }
    }
}

impl SplashState {
    pub fn reset_anim(&mut self, new_job: SplashJob, _lang: Language) {
        self.job = new_job;
        self.done = false;
        self.start_time = None;
        self.fadeout_start = None;
        self.opacity = 1.0;
        self.target_phase = None;
        self.frames_drawn = 0;
        self.gpu_load_step = 0;
        self.progress = 0.0;
        self.visual_progress = 0.0;
        self.last_update_time = None;
        self.random_speed = 0.0;
        self.status_override = None;
        self.status_text = String::new();
    }
}

/// Draw the splash screen. Returns Some(phase) when the splash is fully done and
/// should be dismissed. The caller transitions to the returned phase.
pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut SplashState,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: Language,
) -> Option<crate::app::ClientPhase> {
    state.frames_drawn += 1;
    let now = root_ui.input(|i| i.time);
    root_ui.ctx().request_repaint();

    if state.start_time.is_none() {
        state.start_time = Some(now);
    }

    let elapsed = now - state.start_time.unwrap();
    // Time after fade-in completes
    let hold_elapsed = (elapsed - FADEIN_DURATION).max(0.0);

    // Update visual progress with smooth organic interpolation and randomized speed factor
    let last_time = state.last_update_time.unwrap_or(now);
    state.last_update_time = Some(now);
    let dt = (now - last_time).max(0.0) as f32;

    if state.random_speed == 0.0 {
        // Simple LCG pseudo-random multiplier between 0.7 and 1.3 based on time seed
        let seed = (now * 100_000.0) as u64;
        let r = (seed % 1000) as f32 / 1000.0;
        state.random_speed = 0.7 + r * 0.6;
    }

    // Target caps at 0.9 while loading in background, and only fully finishes to 1.0 when done
    let target = if state.done {
        1.0f32
    } else {
        (state.progress * 0.9).clamp(0.0, 0.9)
    };

    if state.visual_progress < target {
        let diff = target - state.visual_progress;
        let rate = 2.5 * state.random_speed;
        let crawl = 0.03 * state.random_speed;
        let step = (diff * rate + crawl) * dt;
        state.visual_progress = (state.visual_progress + step).min(target);
    } else if state.visual_progress > target {
        state.visual_progress = (state.visual_progress - 2.0 * dt).max(target);
    }

    // Start fadeout once loading is done AND the hold period has elapsed
    if state.done && hold_elapsed >= MIN_HOLD_DURATION && state.fadeout_start.is_none() {
        state.fadeout_start = Some(now);
    }

    // Compute opacity
    if let Some(fade_start) = state.fadeout_start {
        let t = ((now - fade_start) / FADEOUT_DURATION).min(1.0) as f32;
        state.opacity = 1.0 - smoothstep(t);
        if t >= 1.0 {
            return state.target_phase.take();
        }
    } else {
        let t = (elapsed / FADEIN_DURATION).min(1.0) as f32;
        state.opacity = smoothstep(t);
    }

    let visual_progress = state.visual_progress;

    // ponytail: bleed 1px past top to cover any sub-pixel gap
    let mut screen_rect = root_ui.max_rect();
    screen_rect.min.y -= 1.0;
    let screen_w = screen_rect.width();
    let screen_h = screen_rect.height();
    let is_mobile = sow_ui_kit::theme::compact_viewport(root_ui.ctx());
    let alpha = (state.opacity * 255.0) as u8;

    let use_portrait = screen_w < screen_h;
    let background_tex = if use_portrait {
        asset_loader.splash_mobile.as_ref()
    } else {
        asset_loader.splash_desktop.as_ref()
    };

    if let Some(texture) = background_tex {
        let tex_aspect = texture.size()[0] as f32 / texture.size()[1] as f32;
        let screen_aspect = screen_w / screen_h;

        let (mut u0, mut v0, mut u1, mut v1) = (0.0, 0.0, 1.0, 1.0);

        if tex_aspect > screen_aspect {
            let crop_w = screen_aspect / tex_aspect;
            u0 = (1.0 - crop_w) / 2.0;
            u1 = 1.0 - u0;
        } else {
            let crop_h = tex_aspect / screen_aspect;
            v0 = (1.0 - crop_h) / 2.0;
            v1 = 1.0 - v0;
        }

        root_ui.painter().image(
            texture.id(),
            screen_rect,
            egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1)),
            Color32::from_white_alpha(alpha),
        );
    } else {
        root_ui.painter().rect_filled(
            screen_rect,
            0.0,
            Color32::from_rgba_unmultiplied(10, 10, 15, alpha),
        );
    }

    // Loading bar
    let aspect_ratio = 2064.0 / 512.0;
    let base_width = if is_mobile {
        screen_w - 40.0
    } else {
        640.0f32.min(screen_w * 0.65)
    };
    let max_h = (screen_h * 0.13).max(52.0);
    let bar_height = (base_width / aspect_ratio).min(max_h);
    let bar_width = bar_height * aspect_ratio;

    let bottom_padding = (screen_h * 0.14).max(70.0);
    let center_x = screen_rect.center().x;
    let bottom_y = screen_rect.max.y - bottom_padding;

    let bar_rect = egui::Rect::from_center_size(
        egui::pos2(center_x, bottom_y),
        egui::vec2(bar_width, bar_height),
    );

    let tint = Color32::from_white_alpha(alpha);

    if let (Some(empty_tex), Some(full_tex)) =
        (&asset_loader.ui_loader_empty, &asset_loader.ui_loader_full)
    {
        root_ui.painter().image(
            empty_tex.id(),
            bar_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            tint,
        );

        let vp = visual_progress.clamp(0.0, 1.0);
        if vp > 0.0 {
            let mut clip_rect = bar_rect;
            clip_rect.max.x = bar_rect.min.x + bar_width * vp;
            let uv_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(vp, 1.0));
            root_ui
                .painter()
                .image(full_tex.id(), clip_rect, uv_rect, tint);
        }
    } else {
        let vp = visual_progress.clamp(0.0, 1.0);
        let mut fg_rect = bar_rect;
        fg_rect.max.x = bar_rect.min.x + (bar_width * vp).max(bar_height * 0.1);

        root_ui.painter().rect_filled(
            bar_rect,
            bar_height / 2.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, (150.0 * state.opacity) as u8),
        );
        root_ui.painter().rect_filled(
            fg_rect,
            bar_height / 2.0,
            Color32::from_rgba_unmultiplied(0, 180, 255, alpha),
        );
    }

    // Loading text
    let text_color = Color32::from_white_alpha(alpha);
    let shadow_color = Color32::from_rgba_unmultiplied(0, 0, 0, alpha);
    let font_id = egui::FontId::proportional(if is_mobile { 16.0 } else { 18.0 });

    let pct = ((visual_progress * 100.0).clamp(0.0, 100.0)) as i32;
    let status_text = if state.status_text.is_empty() {
        None
    } else {
        Some(state.status_text.as_str())
    };
    let status = state
        .status_override
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(status_text);
    let display_text = if let Some(phase) = status {
        if phase.contains('%') {
            phase.to_string()
        } else {
            format!("{} — {}%", phase, pct)
        }
    } else {
        format!("{} {}%", sow_i18n::get(lang).loading_screen.loading, pct)
    };

    sow_ui_kit::theme::paint_premium_glow_text(
        root_ui.painter(),
        bar_rect.center(),
        egui::Align2::CENTER_CENTER,
        &display_text,
        font_id,
        text_color,
        shadow_color,
    );

    None
}
