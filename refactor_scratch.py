import os
import re

source_path = "sow-client/src/render_frame.rs"
with open(source_path, "r") as f:
    content = f.read()

# We need to extract the parts and build new files.

# 1. Extract imports and setup
lines = content.split('\n')
import_lines = []
impl_start_idx = 0
for i, line in enumerate(lines):
    if line.startswith("impl SowApp {"):
        impl_start_idx = i
        break
    import_lines.append(line)

imports_code = '\n'.join(import_lines)

# Now we find the boundaries inside the `render_frame` method.
# The `run_ui` closure is where we want to substitute our calls.
# Let's locate the `egui_output = self.egui_ctx.run_ui`
run_ui_start = content.find("let egui_output = self.egui_ctx.run_ui(")
world_overlays_start = content.find("let painter = ctx.layer_painter(", run_ui_start)
fleets_start = content.find("// --- Render Fleets ---", world_overlays_start)
dev_ui_start = content.find("self.frame_count += 1;", fleets_start)
fps_start = content.find("egui::Area::new(egui::Id::new(\"fps_counter\"))", dev_ui_start)
interactions_start = content.find("if self.app.phase == ClientPhase::Playing {\n                                    // Check long press", dev_ui_start)
lod_dev_start = content.find("let mut is_expanded = ctx.data_mut(", fps_start)
attacks_panel_start = content.find("if my_pid > 0 && (!snap.attacks.is_empty() || !snap.fleets.is_empty()) {", lod_dev_start)
attacks_panel_outer = content.find("if self.app.phase == ClientPhase::Playing {\n                                    if let Some(snap) = &self.current_snapshot {\n                                        let my_pid = self.my_player_id.unwrap_or(0);", lod_dev_start)

app_draw_start = content.find("if let Some(action) = self.app.draw(ctx) {", attacks_panel_start)
run_ui_end = content.find("});", app_draw_start) + 3

# Wait, the structure inside run_ui is:
# if Playing { world_overlays + fleets + attacks }
# frame_count / ping
# if Playing { check long press / context menu }
# fps counter
# LOD Dev Utils
# if Playing { Attacks panel }
# if let Some(action) = app.draw() ...

# To be perfectly safe, let's just make the python script replace these exact blocks with method calls,
# and write those blocks to the new files wrapped in `impl SowApp { ... }`.

# The python script approach might be complex to get exactly right with indentation.
