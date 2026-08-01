pub fn draw_leader_picker_modal(
    ctx: &egui::Context,
    selected_leader: &mut sow_data::Leader,
    selected_civilization: &mut sow_data::Civilization,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    leader_backdrop: &mut crate::widgets::leader_backdrop::LeaderBackdropTransition,
    lang: sow_i18n::Language,
) -> bool {
    let mut close = false;
    reset_picker_scroll_if_resized(ctx);

    egui::Area::new(egui::Id::new("leader_picker_backdrop"))
        .order(egui::Order::Background)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen_rect = ctx.content_rect();
            let metrics = LeaderPickerMetrics::new(ctx);
            let is_mobile = metrics.is_mobile;

            let content_rect = if is_mobile {
                let mut rect = screen_rect;
                rect.min.x += metrics.content_margin_x;
                rect.max.x -= metrics.content_margin_x;
                rect.min.y += metrics.content_margin_top;
                rect.max.y -= metrics.content_margin_bottom;
                rect
            } else {
                screen_rect.shrink(metrics.content_shrink)
            };

            let is_inside_active_ui = if let Some(click_pos) = ctx.input(|i| {
                i.pointer
                    .press_origin()
                    .or_else(|| i.pointer.interact_pos())
            }) {
                content_rect.contains(click_pos)
            } else {
                false
            };

            // Click backdrop to close
            let backdrop_response = ui.allocate_rect(screen_rect, egui::Sense::click());
            ui.painter()
                .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(210));
            if backdrop_response.clicked() && !is_inside_active_ui {
                close = true;
            }

            let loading_label = &sow_i18n::get(lang).main_menu.loading_leader_portrait;
            let use_portrait = screen_rect.width() < screen_rect.height();
            crate::widgets::leader_backdrop::draw_leader_hero_backdrop(
                ui,
                &mut crate::widgets::leader_backdrop::LeaderHeroBackdropCtx {
                    screen_rect,
                    selected: *selected_leader,
                    mobile: use_portrait,
                    asset_loader,
                    transition: leader_backdrop,
                    loading_label,
                    draw_picker_gradient: true,
                },
            );

            let reign_dates = match *selected_leader {
                sow_data::Leader::Caesar => "Reigned 49 – 44 BC",
                sow_data::Leader::Cleopatra => "Reigned 51 – 30 BC",
                sow_data::Leader::Ragnar => "Reigned 800 – 845 AD",
                sow_data::Leader::SunTzu => "Reigned 544 – 496 BC",
                sow_data::Leader::Alexander => "Reigned 336 – 323 BC",
                sow_data::Leader::GenghisKhan => "Reigned 1206 – 1227 AD",
                sow_data::Leader::RichardTheLionheart => "Reigned 1189 – 1199 AD",
                sow_data::Leader::Vercingetorix => "Reigned 82 – 46 BC",
                sow_data::Leader::Boudica => "Reigned 60 – 61 AD",
                sow_data::Leader::LadySixSky => "Reigned 612 – 693 AD",
                sow_data::Leader::Leonidas => "Reigned 489 – 480 BC",
                sow_data::Leader::Napoleon => "Reigned 1804 – 1814 AD",
            };

            let avatar_size = metrics.avatar_size;
            let scroll_area_h = avatar_size + metrics.scroll_area_pad;
            let carousel_block_h = scroll_area_h;
            let card_w = if is_mobile {
                content_rect.width()
            } else {
                metrics.desktop_text_w
            };
            let confirm_top = content_rect.max.y - metrics.confirm_btn_h - metrics.confirm_gap;
            let confirm_w = if is_mobile {
                metrics.confirm_btn_w_mobile
            } else {
                metrics.confirm_btn_w_desktop
            };
            let confirm_x = if is_mobile {
                content_rect.center().x - confirm_w * 0.5
            } else {
                content_rect.max.x - confirm_w
            };

            let mut desktop_rail: Option<(egui::Rect, f32)> = None;

            ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                if is_mobile {
                    let (back, column_bottom) =
                        draw_leader_picker_top_column(ui, ctx, &metrics, content_rect.max.x);
                    if back {
                        close = true;
                    }

                    // Bottom → top: CONFIRM, avatar carousel, hero text, title (portrait fills middle).
                    let confirm_bottom = content_rect.max.y;
                    let confirm_top_y = confirm_bottom - metrics.confirm_btn_h;
                    let carousel_bottom = confirm_top_y - metrics.confirm_gap;
                    let carousel_top = carousel_bottom - carousel_block_h;
                    let text_bottom = carousel_top - metrics.mobile_stack_gap;
                    let text_top = (text_bottom - metrics.mobile_text_stack_h)
                        .max(column_bottom + metrics.mobile_stack_gap);

                    let carousel_rect = egui::Rect::from_min_max(
                        egui::pos2(content_rect.min.x, carousel_top),
                        egui::pos2(content_rect.max.x, carousel_bottom),
                    );

                    if text_bottom > text_top {
                        let text_rect = egui::Rect::from_min_max(
                            egui::pos2(content_rect.min.x, text_top),
                            egui::pos2(content_rect.max.x, text_bottom),
                        );
                        ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                                ui.set_width(card_w);
                                draw_leader_hero_text(
                                    ui,
                                    *selected_leader,
                                    *selected_civilization,
                                    reign_dates,
                                    card_w,
                                    &metrics,
                                    true,
                                );
                            });
                        });
                    }

                    ui.scope_builder(egui::UiBuilder::new().max_rect(carousel_rect), |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.set_width(content_rect.width());
                            draw_leader_carousel(
                                ui,
                                selected_leader,
                                selected_civilization,
                                asset_loader,
                                &metrics,
                                scroll_area_h,
                                content_rect.width(),
                            );
                        });
                    });
                } else {
                    let is_desktop_narrow = screen_rect.width() < DESKTOP_NARROW_BREAKPOINT;
                    let back_rect =
                        crate::ui::main_menu::profile::main_menu_avatar_button_rect(ctx);
                    let (back, column_bottom) =
                        draw_leader_picker_top_column(ui, ctx, &metrics, content_rect.max.x);
                    if back {
                        close = true;
                    }

                    let avatar_lane_x = back_rect.min.x;
                    let pick_top = column_bottom.max(back_rect.max.y) + metrics.rail_header_gap;
                    let pick_bottom = confirm_top - metrics.confirm_gap;
                    let pick_h = (pick_bottom - pick_top).max(avatar_size + metrics.rail_min_extra);
                    let rail_rect = egui::Rect::from_min_max(
                        egui::pos2(avatar_lane_x - metrics.rail_scroll_extent(), pick_top),
                        egui::pos2(avatar_lane_x + avatar_size, pick_bottom),
                    );
                    desktop_rail = Some((rail_rect, pick_h));

                    if is_desktop_narrow {
                        let hero_left = avatar_lane_x + avatar_size + metrics.rail_text_gap;
                        let hero_right = (hero_left + card_w).min(content_rect.max.x);
                        let hero_rect = egui::Rect::from_min_max(
                            egui::pos2(
                                hero_left,
                                confirm_top - metrics.confirm_gap - metrics.desktop_narrow_hero_h,
                            ),
                            egui::pos2(hero_right, confirm_top - metrics.confirm_gap),
                        );
                        if hero_rect.height() > 0.0 && hero_rect.width() > 0.0 {
                            ui.scope_builder(egui::UiBuilder::new().max_rect(hero_rect), |ui| {
                                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                                    ui.set_width(hero_rect.width());
                                    draw_leader_hero_text(
                                        ui,
                                        *selected_leader,
                                        *selected_civilization,
                                        reign_dates,
                                        hero_rect.width(),
                                        &metrics,
                                        false,
                                    );
                                });
                            });
                        }
                    } else {
                        let text_rect = egui::Rect::from_min_max(
                            egui::pos2(
                                avatar_lane_x + avatar_size + metrics.rail_text_gap,
                                pick_top,
                            ),
                            egui::pos2(
                                (avatar_lane_x + avatar_size + metrics.rail_text_gap + card_w)
                                    .min(content_rect.max.x),
                                pick_top + pick_h,
                            ),
                        );

                        ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                                ui.set_width(text_rect.width());
                                draw_leader_hero_text(
                                    ui,
                                    *selected_leader,
                                    *selected_civilization,
                                    reign_dates,
                                    text_rect.width(),
                                    &metrics,
                                    false,
                                );
                            });
                        });
                    }
                }

                let confirm_rect = egui::Rect::from_min_max(
                    egui::pos2(confirm_x, content_rect.max.y - metrics.confirm_btn_h),
                    egui::pos2(confirm_x + confirm_w, content_rect.max.y),
                );
                let confirm_button = if is_mobile {
                    crate::widgets::ThemeButton::new("CONFIRM")
                        .style(crate::widgets::ThemeButtonStyle::Primary)
                        .min_size(egui::vec2(confirm_w, metrics.confirm_btn_h))
                        .text_size(metrics.confirm_text_size)
                } else {
                    crate::widgets::ThemeButton::new("CONFIRM")
                        .custom_fill(Color32::from_black_alpha(175))
                        .min_size(egui::vec2(confirm_w, metrics.confirm_btn_h))
                        .text_size(metrics.confirm_text_size)
                };
                let confirm_response = ui.put(confirm_rect, confirm_button);
                if confirm_response.clicked() {
                    close = true;
                }
            });

            if let Some((rail_rect, pick_h)) = desktop_rail {
                ui.scope_builder(egui::UiBuilder::new().max_rect(rail_rect), |ui| {
                    draw_leader_rail(
                        ui,
                        selected_leader,
                        selected_civilization,
                        asset_loader,
                        &metrics,
                        pick_h,
                    );
                });
            }
        });

    close
}
