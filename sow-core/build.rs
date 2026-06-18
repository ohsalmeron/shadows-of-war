/// Must match `sow-dist/src/assets.rs` `UI_FONT_FILE` and `sow-ui/src/ui_font.rs`.
const UI_FONT_FILE: &str = "WorkSans-Black.ttf";

fn main() {
    println!("cargo:rerun-if-changed=../assets/static/maps/world");
    let map_path = "../assets/static/maps/world/map.bin.br";
    println!("cargo:rerun-if-changed={map_path}");

    let font_path = format!("../assets/static/fonts/{UI_FONT_FILE}");
    println!("cargo:rerun-if-changed={font_path}");
    if !std::path::Path::new(&font_path).is_file() {
        panic!(
            "missing assets/static/fonts/{UI_FONT_FILE}\n\
             required for compile-time UI font embed (sow-ui/src/ui_font.rs)\n\
             download from Google Fonts or restore from git"
        );
    }
}
