//! Poka-yoke guard: emoji must flow through the SOW emoji pipeline, never raw egui font.
//!
//! The mistake this catches: dropping an emoji straight into an egui text widget —
//! `ui.label("Building")`, `RichText::new("...")`, `painter.text(.. emoji ..)` — which
//! lays it out with the UI font (tofu / monochrome) instead of the color atlas. The
//! unified pipeline (`prepare_name` for egui, `push_string`/`push_emoji` for GPU) routes
//! every emoji through the atlas; going around it is the bug.
//!
//! The right way:
//!   * egui : sow_ui_kit::widgets::emoji::{emoji_label, outlined_emoji_label,
//!            try_paint_emoji, paint_emoji_centered, paint_emoji_text_at, HudEmojiButton}
//!   * GPU  : sow_render::TextRenderer::{push_emoji, push_string}
//!   * new glyph: add it to the atlas (sow-data/src/emoji/manifest.rs, regen via sow-tools).
//!
//! Escape hatch: append `// emoji-ok` to a line that is an intentional font fallback
//! (e.g. the `painter.text` branch taken only when `try_paint_emoji` returns false).

use std::fs;
use std::path::{Path, PathBuf};

use sow_data::emoji::lookup;

/// egui text widgets/constructors that lay glyphs out with the UI font (no color atlas).
const RAW_FONT_SINKS: &[&str] = &[
    ".label(",
    ".button(",
    ".heading(",
    ".small(",
    ".monospace(",
    ".code(",
    ".strong(",
    ".weak(",
    ".selectable_label(",
    ".checkbox(",
    ".radio(",
    ".radio_value(",
    ".toggle_value(",
    ".collapsing(",
    "RichText::new(",
    "Label::new(",
    "Button::new(",
    "CollapsingHeader::new(",
    "painter.text(",
];

/// Approved emoji entry points: these route through the atlas (`prepare_name` /
/// `try_paint_emoji`). An emoji passed here must still exist in the atlas, otherwise it
/// silently falls back to the UI font — so we check membership instead of banning it.
/// `ThemeButton` / `HudEmojiButton` qualify: both paint their label via the atlas pipeline.
const EMOJI_APIS: &[&str] = &[
    "push_emoji(",
    "push_string(",
    "try_paint_emoji(",
    "paint_emoji_centered(",
    "paint_emoji_text_at(",
    "outlined_emoji_text(",
    "emoji_label(",
    "outlined_emoji_label(",
    "HudEmojiButton::new(",
    "ThemeButton::new(",
];

/// Append this to a line to mark an intentional, reviewed font fallback.
const ALLOW_MARKER: &str = "emoji-ok";

/// Color/pictographic emoji codepoints (deliberately excludes plain arrows like `←`/`→`
/// and letterlike symbols that the UI font renders fine).
fn is_emoji_char(c: char) -> bool {
    let u = c as u32;
    matches!(u,
        0x1F000..=0x1FAFF | // emoticons, pictographs, transport, symbols, supplemental
        0x2600..=0x27BF   | // misc symbols + dingbats (atom, anchor, gear, arrows, checks…)
        0x2B00..=0x2BFF   | // star, up/down arrows used as emoji
        0xFE0F            | // VS16 emoji-presentation selector — a strong "meant as emoji" signal
        0x20E3              // combining enclosing keycap
    )
}

fn line_has_emoji(line: &str) -> bool {
    line.chars().any(is_emoji_char)
}

/// Mirror the runtime splitter (`match_emoji_at`): a run is supported if any char-prefix
/// of it resolves in the atlas (lookup is tried on progressively longer candidates).
fn run_supported(run: &str) -> bool {
    let chars: Vec<char> = run.chars().collect();
    (1..=chars.len()).any(|n| {
        let cand: String = chars[..n].iter().collect();
        lookup(&cand).is_some()
    })
}

/// Maximal runs of emoji chars, keeping ZWJ/VS16 joiners attached.
fn emoji_runs(s: &str) -> Vec<String> {
    let mut runs = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        let joiner = c as u32 == 0x200D; // ZWJ holds a sequence together
        if is_emoji_char(c) || (joiner && !cur.is_empty()) {
            cur.push(c);
        } else if !cur.is_empty() {
            runs.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
    }
    runs
}

/// Substring match that, for identifier-starting tokens, respects a left identifier
/// boundary so `Button::new(` does not match inside `HudEmojiButton::new(` /
/// `ThemeButton::new(`. Method tokens like `.label(` start with `.`, which is already a
/// boundary (the receiver before it is expected), so they match plainly. (All tokens end
/// in `(`, so the right side is always bounded.)
fn contains_token(line: &str, token: &str) -> bool {
    let starts_with_ident = token
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_');
    if !starts_with_ident {
        return line.contains(token);
    }
    let mut from = 0;
    while let Some(off) = line[from..].find(token) {
        let idx = from + off;
        let prev_is_ident = line[..idx]
            .chars()
            .next_back()
            .is_some_and(|p| p.is_alphanumeric() || p == '_');
        if !prev_is_ident {
            return true;
        }
        from = idx + token.len();
    }
    false
}

/// First double-quoted string literal on a line (emoji literals never contain escaped quotes).
fn first_str_literal(line: &str) -> Option<&str> {
    let start = line.find('"')? + 1;
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn collect_rs(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn rel(p: &Path, base: &Path) -> String {
    p.strip_prefix(base).unwrap_or(p).display().to_string()
}

#[test]
fn emoji_goes_through_the_pipeline() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")); // .../sow-ui-kit
    let workspace = manifest.parent().expect("workspace root");
    let roots = [
        workspace.join("sow-client-world/src"),
        workspace.join("sow-ui/src"),
        workspace.join("sow-ui-kit/src"),
    ];

    let mut files = Vec::new();
    for r in &roots {
        collect_rs(r, &mut files);
    }
    assert!(
        !files.is_empty(),
        "emoji guard found no source files — workspace layout changed?"
    );

    let mut violations = Vec::new();

    for file in &files {
        let Ok(src) = fs::read_to_string(file) else {
            continue;
        };
        for (i, line) in src.lines().enumerate() {
            if line.contains(ALLOW_MARKER) {
                continue;
            }
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with('*') {
                continue; // comment / doc line
            }
            let lineno = i + 1;
            let approved = EMOJI_APIS.iter().any(|s| contains_token(line, s));

            // (A) Raw emoji dumped into an egui font widget (skip lines that already
            // route through an approved atlas entry point).
            if !approved
                && line_has_emoji(line)
                && RAW_FONT_SINKS.iter().any(|s| contains_token(line, s))
            {
                violations.push(format!(
                    "{}:{}: raw emoji into an egui font widget\n      {}",
                    rel(file, workspace),
                    lineno,
                    line.trim()
                ));
                continue;
            }

            // (B) Emoji handed to an approved API but missing from the atlas.
            if approved {
                if let Some(lit) = first_str_literal(line) {
                    for run in emoji_runs(lit) {
                        if !run_supported(&run) {
                            violations.push(format!(
                                "{}:{}: emoji {:?} is not in the atlas (renders as font fallback)\n      {}",
                                rel(file, workspace),
                                lineno,
                                run,
                                line.trim()
                            ));
                        }
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "\n\nSOW emoji pipeline guard tripped — {} violation(s):\n\n{}\n\n\
         Emoji must NOT render raw through egui's font. Route them through the pipeline:\n\
         \x20 egui : sow_ui_kit::widgets::emoji::{{emoji_label, outlined_emoji_label, try_paint_emoji,\n\
         \x20         paint_emoji_centered, paint_emoji_text_at, HudEmojiButton}}\n\
         \x20 GPU  : sow_render::TextRenderer::{{push_emoji, push_string}}\n\
         \x20 new glyph? add it to the atlas: sow-data/src/emoji/manifest.rs (regen via sow-tools).\n\
         Intentional font fallback? append  // emoji-ok  to the line.\n",
        violations.len(),
        violations.join("\n\n"),
    );
}
