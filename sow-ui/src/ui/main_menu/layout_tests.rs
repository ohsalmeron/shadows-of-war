//! Exercise the actual egui widgets across resize transitions, not just the
//! layout arithmetic: a child frame can silently expand its parent.
use super::*;
use egui::{Color32, Rect, Shape, pos2, vec2};

fn rects(shape: &Shape, out: &mut Vec<egui::epaint::RectShape>) {
    match shape {
        Shape::Rect(rect) => out.push(rect.clone()),
        Shape::Vec(shapes) => shapes.iter().for_each(|s| rects(s, out)),
        _ => {}
    }
}

#[test]
fn home_resize_keeps_panel_bound_to_map_and_clear_of_footer() {
    let ctx = egui::Context::default();
    crate::theme::apply_theme(&ctx);
    let mut assets = crate::ui::asset_loader::AssetLoader::new();
    let texture = ctx.load_texture(
        "test_map",
        egui::ColorImage::filled([16, 9], Color32::BLUE),
        Default::default(),
    );
    let texture_id = texture.id();
    assets.thumbnails.insert("world".into(), texture);
    let mut state = MainMenuState {
        is_connected: true,
        ..Default::default()
    };
    state.lobbies.push(LobbyInfo {
        id: 1,
        num_players: 8,
        max_players: 15,
        is_counting_down: true,
        timer_secs: 8.0,
        map_name: "world".into(),
        game_mode: "Teams".into(),
        players: vec![],
        has_password: false,
        host_name: String::new(),
        bot_count: 420,
        nation_count: 128,
        bot_difficulty: Default::default(),
        kind: sow_core::protocol::LobbyKind::Matchmaking,
    });

    // Includes both sides of the portrait breakpoint and a return to desktop.
    for (w, h) in [
        (1440.0, 810.0),
        (800.0, 760.0),
        (760.0, 800.0),
        (610.0, 780.0),
        (390.0, 844.0),
        (360.0, 640.0),
        (600.0, 540.0),
        (1920.0, 1080.0),
        (1440.0, 810.0),
    ] {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h));
        let mut painted = Vec::new();
        // Allow egui's footer sizing to settle after each resize.
        for _ in 0..3 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| {
                    super::draw(
                        ui,
                        &mut state,
                        &mut assets,
                        sow_i18n::Language::English,
                        true,
                    );
                },
            );
            painted.clear();
            for clipped in &output.shapes {
                rects(&clipped.shape, &mut painted);
            }
        }
        let panel = painted
            .iter()
            .find(|r| r.fill == Color32::from_rgba_unmultiplied(9, 11, 15, 190))
            .expect("command panel must be drawn")
            .rect;
        let map = painted
            .iter()
            .find(|r| {
                r.brush
                    .as_ref()
                    .is_some_and(|b| b.fill_texture_id == texture_id)
            })
            .expect("live map must be drawn")
            .rect;
        let phone = layout::viewport_class(w, h) == layout::ViewportClass::Phone;
        let footer = painted.iter().find(|r| {
            r.fill == crate::theme::palette::surface()
                && (r.rect.bottom() - h).abs() < 1.0
                && r.rect.height() <= 30.0
        });
        let mobile_nav = painted.iter().find(|r| {
            r.fill == Color32::from_rgba_unmultiplied(7, 9, 13, 242)
                && (r.rect.bottom() - h).abs() < 1.0
                && r.rect.height() >= 60.0
        });

        assert!(
            (map.width() / map.height() - 16.0 / 9.0).abs() < 0.02,
            "{w}x{h}: {map:?}"
        );
        assert!(
            (panel.width() - map.width() - 34.0).abs() < 1.0,
            "{w}x{h}: panel {panel:?} expanded beyond map {map:?}"
        );
        if phone {
            assert!(
                (panel.center().x - w * 0.5).abs() < 1.0,
                "portrait is not centered: {panel:?}"
            );
            let nav = mobile_nav
                .expect("portrait must have one fixed bottom nav row")
                .rect;
            assert!(
                nav.height() <= 74.0,
                "portrait nav expanded into multiple rows: {nav:?}"
            );
            assert!(
                panel.bottom() <= nav.top() - 11.0,
                "portrait panel touches bottom nav: {panel:?} / {nav:?}"
            );
        } else {
            let pad = layout::main_menu_metrics(&ctx).outer_pad;
            assert!(
                panel.left() >= 96.0 + pad - 2.0,
                "desktop panel escaped the fixed rail: {panel:?}"
            );
            if h >= 810.0 {
                assert!(
                    (map.width() - 560.0).abs() < 1.0,
                    "approved desktop map size changed: {map:?}"
                );
            }
        }
        if let Some(footer) = footer {
            assert!(
                panel.bottom() <= footer.rect.top() - 11.0,
                "{w}x{h}: panel {panel:?} touches footer {footer:?}"
            );
            assert!(footer.rect.height() <= 30.0, "footer wrapped: {footer:?}");
        }
        let actions: Vec<_> = painted
            .iter()
            .filter(|r| {
                r.rect.top() >= map.bottom()
                    && r.rect.bottom() <= panel.bottom()
                    && r.rect.width() > map.width() * 0.8
            })
            .collect();
        assert!(
            actions.len() >= 4,
            "all four action rows must remain visible"
        );
        for action in actions {
            assert!(
                (action.rect.left() - map.left()).abs() < 1.0
                    && (action.rect.right() - map.right()).abs() < 1.0,
                "{w}x{h}: action {:?} does not match map {map:?}",
                action.rect
            );
        }
    }
}

#[test]
fn home_rotation_keeps_quick_match_geometry_stable() {
    let ctx = egui::Context::default();
    crate::theme::apply_theme(&ctx);
    let mut assets = crate::ui::asset_loader::AssetLoader::new();
    let texture = ctx.load_texture(
        "rotation_map",
        egui::ColorImage::filled([16, 9], Color32::BLUE),
        Default::default(),
    );
    let texture_id = texture.id();
    assets.thumbnails.insert("world".into(), texture);
    let mut state = MainMenuState {
        is_connected: true,
        ..Default::default()
    };

    let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1440.0, 810.0));
    let mut expected: Option<(Rect, Rect, Rect, Vec<Rect>)> = None;

    for lobbies in [
        vec![LobbyInfo {
            id: 1,
            num_players: 8,
            max_players: 15,
            is_counting_down: true,
            timer_secs: 8.0,
            map_name: "world".into(),
            game_mode: "FFA".into(),
            players: vec![],
            has_password: false,
            host_name: String::new(),
            bot_count: 420,
            nation_count: 128,
            bot_difficulty: Default::default(),
            kind: sow_core::protocol::LobbyKind::Matchmaking,
        }],
        vec![LobbyInfo {
            id: 2,
            num_players: 4,
            max_players: 10,
            is_counting_down: true,
            timer_secs: 8.0,
            map_name: "rotated_map".into(),
            game_mode: "Teams".into(),
            players: vec![],
            has_password: false,
            host_name: String::new(),
            bot_count: 420,
            nation_count: 128,
            bot_difficulty: Default::default(),
            kind: sow_core::protocol::LobbyKind::Matchmaking,
        }],
        Vec::new(),
    ] {
        state.lobbies = lobbies;
        let mut painted = Vec::new();
        for _ in 0..3 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| {
                    super::draw(
                        ui,
                        &mut state,
                        &mut assets,
                        sow_i18n::Language::English,
                        true,
                    );
                },
            );
            painted.clear();
            for clipped in &output.shapes {
                rects(&clipped.shape, &mut painted);
            }
        }

        let panel = painted
            .iter()
            .find(|r| r.fill == Color32::from_rgba_unmultiplied(9, 11, 15, 190))
            .expect("command panel must be drawn")
            .rect;
        let footer = painted
            .iter()
            .find(|r| {
                r.fill == crate::theme::palette::surface() && (r.rect.bottom() - 810.0).abs() < 1.0
            })
            .expect("footer must be drawn")
            .rect;
        let map = painted
            .iter()
            .find(|r| {
                r.brush
                    .as_ref()
                    .is_some_and(|b| b.fill_texture_id == texture_id)
                    || (r.fill == crate::theme::palette::button_inactive()
                        && r.rect.width() > 300.0
                        && r.rect.height() > 200.0)
            })
            .expect("quick-match slot must be drawn")
            .rect;
        let actions: Vec<Rect> = painted
            .iter()
            .filter(|r| {
                r.rect.top() >= map.bottom()
                    && r.rect.bottom() <= panel.bottom()
                    && r.rect.width() > map.width() * 0.8
                    && r.rect.height() >= 35.0
                    && r.rect.height() <= 50.0
            })
            .map(|r| r.rect)
            .collect();

        if let Some((old_panel, old_map, old_footer, old_actions)) = expected.as_ref() {
            assert_eq!(panel, *old_panel, "panel moved during lobby rotation");
            assert_eq!(map, *old_map, "map slot changed during lobby rotation");
            assert_eq!(footer, *old_footer, "footer moved during lobby rotation");
            assert_eq!(
                &actions, old_actions,
                "action rows reflowed during lobby rotation"
            );
        } else {
            expected = Some((panel, map, footer, actions));
        }
    }
}

#[test]
fn waiting_keeps_queue_alive_while_navigation_changes_sections() {
    let mut state = MainMenuState {
        is_waiting: true,
        ..Default::default()
    };

    assert_eq!(state.visible_route(), MainMenuRoute::Queue);
    assert_eq!(state.active_section(), MainMenuSection::Battle);

    state.open_section(MainMenuSection::Store);
    assert_eq!(state.visible_route(), MainMenuRoute::Store);
    assert_eq!(state.active_section(), MainMenuSection::Store);

    state.open_section(MainMenuSection::Profile);
    assert_eq!(state.visible_route(), MainMenuRoute::Profile);
    assert_eq!(state.active_section(), MainMenuSection::Profile);

    state.open_section(MainMenuSection::Battle);
    assert_eq!(state.visible_route(), MainMenuRoute::Queue);
    assert_eq!(state.active_section(), MainMenuSection::Battle);
}
