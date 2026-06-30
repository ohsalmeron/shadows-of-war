//! UI Scene Editor — JSON → Rust codegen ("Export").
//!
//! Takes `assets/ui/<scene>.json` (the only source of truth, authored by the web editor) and
//! generates `sow-client/src/ui_scene/generated/<scene>.rs` — real code calling the same
//! `theme::*` components the rest of the client uses, so the compiled result is 1:1 with the
//! game, not an approximation. Mirrors the shape of the hand-written exemplar
//! `sow-client/src/ui_scene/generated/sample.rs`; keep them in sync if that shape changes.
//!
//! Also regenerates `generated/mod.rs` (the scene registry) by scanning `assets/ui/*.json`, so
//! adding a scene never requires a hand edit to the registry.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct SceneDef {
    name: String,
    root: Node,
}

#[derive(Deserialize)]
struct Size {
    w: f32,
    h: f32,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Node {
    Panel {
        preset: String,
        size: Size,
        children: Vec<Node>,
    },
    Text {
        text: String,
        size: f32,
        color: String,
        #[serde(default)]
        strong: bool,
    },
    Button {
        text: String,
        size: f32,
        color: String,
    },
    Spacer {
        h: f32,
    },
}

/// theme::palette tokens the editor may reference by name (kept in sync with the audited
/// palette — see `sow-ui/src/kit/theme/palette.rs`).
const KNOWN_COLORS: &[&str] = &[
    "surface",
    "neon_cyan",
    "neon_cyan_hover",
    "neon_cyan_glow",
    "neon_gold",
    "neon_gold_hover",
    "button_inactive",
    "button_hovered",
    "field_bg",
    "field_border",
    "danger",
    "danger_border",
    "pink",
    "text_normal",
    "text_muted",
];

/// theme panel-frame presets the editor may reference by name.
const KNOWN_PRESETS: &[&str] = &["hud_panel_frame", "leaderboard_panel_frame"];

fn resolve_color(token: &str) -> Result<String> {
    if KNOWN_COLORS.contains(&token) {
        return Ok(format!("theme::palette::{token}()"));
    }
    if let Some(hex) = token.strip_prefix('#') {
        let (r, g, b, a) = parse_hex_color(hex)
            .with_context(|| format!("color '#{hex}' must be 6 or 8 hex digits"))?;
        return Ok(format!("egui::Color32::from_rgba_unmultiplied({r}, {g}, {b}, {a})"));
    }
    bail!(
        "unknown color '{token}' — use a theme::palette name ({}) or #rrggbb[aa]",
        KNOWN_COLORS.join(", ")
    )
}

fn parse_hex_color(hex: &str) -> Result<(u8, u8, u8, u8)> {
    let bytes = match hex.len() {
        6 => (
            u8::from_str_radix(&hex[0..2], 16)?,
            u8::from_str_radix(&hex[2..4], 16)?,
            u8::from_str_radix(&hex[4..6], 16)?,
            255,
        ),
        8 => (
            u8::from_str_radix(&hex[0..2], 16)?,
            u8::from_str_radix(&hex[2..4], 16)?,
            u8::from_str_radix(&hex[4..6], 16)?,
            u8::from_str_radix(&hex[6..8], 16)?,
        ),
        n => bail!("expected 6 or 8 hex digits, got {n}"),
    };
    Ok(bytes)
}

fn resolve_preset(token: &str) -> Result<&'static str> {
    match token {
        "hud_panel_frame" => Ok("theme::hud_panel_frame()"),
        "leaderboard_panel_frame" => Ok("theme::leaderboard_panel_frame()"),
        other => bail!(
            "unknown panel preset '{other}' — use one of: {}",
            KNOWN_PRESETS.join(", ")
        ),
    }
}

/// Render one non-root node as a line of Rust source, indented to sit inside the
/// `vertical_centered` closure the generated module always opens with.
fn render_child(node: &Node, out: &mut String) -> Result<()> {
    const PAD: &str = "                    ";
    match node {
        Node::Text {
            text,
            size,
            color,
            strong,
        } => {
            let color_call = resolve_color(color)?;
            let strong_call = if *strong { ".strong()" } else { "" };
            out.push_str(&format!(
                "{PAD}ui.label(RichText::new({text:?}).size({size:.1}){strong_call}.color({color_call}));\n"
            ));
        }
        Node::Button { text, size, color } => {
            let color_call = resolve_color(color)?;
            out.push_str(&format!(
                "{PAD}let _ = ui.button(RichText::new({text:?}).size({size:.1}).color({color_call}));\n"
            ));
        }
        Node::Spacer { h } => {
            out.push_str(&format!("{PAD}ui.add_space({h:.1});\n"));
        }
        Node::Panel { .. } => bail!("nested panels are not supported yet — keep the root panel flat"),
    }
    Ok(())
}

fn render_scene_module(def: &SceneDef) -> Result<String> {
    let Node::Panel {
        preset,
        size,
        children,
    } = &def.root
    else {
        bail!("scene root must be a panel node");
    };
    let preset_call = resolve_preset(preset)?;

    let mut body = String::new();
    for child in children {
        render_child(child, &mut body)?;
    }

    Ok(format!(
        r#"//! GENERATED by `./sow ui` export from `assets/ui/{name}.json` — do not edit by hand.
//! Regenerate via the UI Scene Editor; this file is overwritten on every export.

use egui::{{pos2, Id, RichText}};
use sow_ui_kit::theme;

pub fn render(ctx: &egui::Context) {{
    let screen = ctx.content_rect();
    let panel_w = {w:.1}_f32;
    let panel_h = {h:.1}_f32;
    let top_left = pos2(
        screen.center().x - panel_w * 0.5,
        screen.center().y - panel_h * 0.5,
    );

    egui::Area::new(Id::new("ui_scene.{name}.root"))
        .fixed_pos(top_left)
        .show(ctx, |ui| {{
            ui.set_width(panel_w);
            {preset_call}.show(ui, |ui| {{
                ui.set_width(panel_w);
                ui.vertical_centered(|ui| {{
                    ui.add_space(8.0);
{body}                    ui.add_space(10.0);
                }});
            }});
        }});
}}
"#,
        name = def.name,
        w = size.w,
        h = size.h,
    ))
}

/// Scan `assets/ui/*.json` and regenerate `generated/mod.rs` — the `mod` list, the name→render
/// dispatch, and `names()`. `sample` is always included (the hand-written exemplar); a
/// json-defined scene is only listed once its `<name>.rs` has actually been exported, so a
/// json file someone is still drafting doesn't break the registry.
fn regenerate_registry(repo_root: &Path, generated_dir: &Path) -> Result<()> {
    let ui_dir = repo_root.join("assets/ui");
    let mut scenes: Vec<String> = vec!["sample".to_string()];
    if ui_dir.is_dir() {
        for entry in std::fs::read_dir(&ui_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if stem != "sample" && generated_dir.join(format!("{stem}.rs")).is_file() {
                scenes.push(stem.to_string());
            }
        }
    }
    scenes.sort();
    scenes.dedup();

    let mods: String = scenes.iter().map(|s| format!("mod {s};\n")).collect();
    let arms: String = scenes
        .iter()
        .map(|s| format!("        \"{s}\" => {{\n            {s}::render(ctx);\n            true\n        }}\n"))
        .collect();
    let names: String = scenes
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");

    let content = format!(
        r#"//! Generated UI scenes — **build artifacts**, not hand-authored source.
//!
//! Each `<scene>.rs` here is emitted by the UI Scene Editor's exporter from
//! `assets/ui/<scene>.json`. Treat these files as generated: regenerate via `./sow ui` → Export,
//! don't hand-edit (your edits are overwritten on the next export). The JSON is the only source
//! of truth — exactly the discipline `campaign/boudica.rs` follows for the roster.
//!
//! `sample.rs` is the one exception: a committed hand-written exemplar that proves the call site
//! and the codegen *shape* the exporter must match. THIS FILE is also regenerated — its
//! `mod`/`match`/`names()` are derived from `assets/ui/*.json` on every export, so adding a scene
//! never requires a hand edit here.

{mods}
/// Dispatch to a generated scene by name. Returns `false` if no scene with that name is
/// registered (so the caller can surface a hint instead of rendering nothing).
pub fn render(name: &str, ctx: &egui::Context) -> bool {{
    match name {{
{arms}        _ => false,
    }}
}}

/// Names of all registered scenes — used for the "unknown scene" hint and tests. Kept in sync by
/// the exporter; do not hand-edit.
pub fn names() -> &'static [&'static str] {{
    &[{names}]
}}
"#
    );
    std::fs::write(generated_dir.join("mod.rs"), content)?;
    Ok(())
}

/// Export `assets/ui/<scene>.json` → `sow-client/src/ui_scene/generated/<scene>.rs`, then
/// regenerate the registry so the new scene is reachable. Returns the path written.
pub fn export_scene(repo_root: &Path, scene: &str) -> Result<PathBuf> {
    if scene.is_empty() || !scene.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("scene name must be alphanumeric/underscore, got '{scene}'");
    }
    let json_path = repo_root.join("assets/ui").join(format!("{scene}.json"));
    let data = std::fs::read_to_string(&json_path)
        .with_context(|| format!("reading {}", json_path.display()))?;
    let def: SceneDef = serde_json::from_str(&data)
        .with_context(|| format!("parsing {}", json_path.display()))?;
    if def.name != scene {
        bail!(
            "scene '{}' field in {} does not match file name '{scene}'",
            def.name,
            json_path.display()
        );
    }

    let rs = render_scene_module(&def)?;
    let generated_dir = repo_root.join("sow-client/src/ui_scene/generated");
    std::fs::create_dir_all(&generated_dir)?;
    let out_path = generated_dir.join(format!("{scene}.rs"));
    std::fs::write(&out_path, rs)?;

    regenerate_registry(repo_root, &generated_dir)?;
    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_demo_scene(root: &Path) {
        std::fs::create_dir_all(root.join("assets/ui")).unwrap();
        std::fs::write(
            root.join("assets/ui/demo.json"),
            r##"{
              "name": "demo",
              "root": {
                "kind": "panel",
                "preset": "hud_panel_frame",
                "size": { "w": 300, "h": 150 },
                "children": [
                  { "kind": "text", "text": "Demo", "size": 20.0, "color": "neon_gold", "strong": true },
                  { "kind": "spacer", "h": 6.0 },
                  { "kind": "text", "text": "Body", "size": 12.0, "color": "text_muted" },
                  { "kind": "button", "text": "OK", "size": 14.0, "color": "#ff8800" }
                ]
              }
            }"##,
        )
        .unwrap();
    }

    #[test]
    fn exports_scene_and_registers_it() {
        let tmp = tempfile::tempdir().unwrap();
        write_demo_scene(tmp.path());

        let out = export_scene(tmp.path(), "demo").unwrap();
        let rs = std::fs::read_to_string(&out).unwrap();
        assert!(rs.contains("theme::hud_panel_frame()"));
        assert!(rs.contains("\"Demo\""));
        assert!(rs.contains("theme::palette::neon_gold()"));
        assert!(rs.contains("egui::Color32::from_rgba_unmultiplied(255, 136, 0, 255)"));

        let mod_rs = std::fs::read_to_string(
            tmp.path().join("sow-client/src/ui_scene/generated/mod.rs"),
        )
        .unwrap();
        assert!(mod_rs.contains("mod demo;"));
        assert!(mod_rs.contains("\"demo\" =>"));
        assert!(mod_rs.contains("mod sample;"), "sample must stay registered");
    }

    #[test]
    fn rejects_name_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        write_demo_scene(tmp.path());
        // demo.json declares name "demo"; exporting under a different scene name must fail.
        std::fs::copy(
            tmp.path().join("assets/ui/demo.json"),
            tmp.path().join("assets/ui/other.json"),
        )
        .unwrap();
        assert!(export_scene(tmp.path(), "other").is_err());
    }

    #[test]
    fn rejects_unknown_color() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("assets/ui")).unwrap();
        std::fs::write(
            tmp.path().join("assets/ui/bad.json"),
            r#"{"name":"bad","root":{"kind":"panel","preset":"hud_panel_frame",
               "size":{"w":100,"h":100},"children":[
               {"kind":"text","text":"x","size":10.0,"color":"not_a_real_color"}]}}"#,
        )
        .unwrap();
        assert!(export_scene(tmp.path(), "bad").is_err());
    }
}
