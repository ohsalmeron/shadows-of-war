//! Generated UI scenes — **build artifacts**, not hand-authored source.
//!
//! Each `<scene>.rs` here is emitted by the UI Scene Editor's exporter from
//! `assets/ui/<scene>.json`. Treat these files as generated: regenerate via `./sow ui` → Export,
//! don't hand-edit (your edits are overwritten on the next export). The JSON is the only source
//! of truth — exactly the discipline `campaign/boudica.rs` follows for the roster.
//!
//! `sample.rs` is the one exception: a committed hand-written exemplar that proves the call site
//! and the codegen *shape* the exporter must match (M0). The generator will produce modules in
//! this same form.

mod sample;

/// Dispatch to a generated scene by name. Returns `false` if no scene with that name is
/// registered (so the caller can surface a hint instead of rendering nothing).
pub fn render(name: &str, ctx: &egui::Context) -> bool {
    match name {
        "sample" => {
            sample::render(ctx);
            true
        }
        _ => false,
    }
}

/// Names of all registered scenes — used for the "unknown scene" hint and tests. The exporter
/// keeps this list in sync when it (re)generates a scene module.
pub fn names() -> &'static [&'static str] {
    &["sample"]
}
