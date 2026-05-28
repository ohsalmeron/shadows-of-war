fn main() {
    println!("cargo:rerun-if-changed=../assets/maps");
    // Bundled include_bytes! for world / tutorial / giantworldmap
    for key in ["world", "giantworldmap", "tutorial"] {
        let path = format!("../assets/maps/{key}/map.bin.br");
        println!("cargo:rerun-if-changed={path}");
    }
}
