fn main() {
    let mut total = 0;
    let mut success = 0;
    for entry in std::fs::read_dir("../assets/maps").unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            let path = entry.path().join("manifest.json");
            if let Ok(data) = std::fs::read_to_string(&path) {
                total += 1;
                match serde_json::from_str::<sow_core::map_legacy::MapManifest>(&data) {
                    Ok(man) => {
                        println!(
                            "Parsed OK: {} ({} nations)",
                            entry.file_name().to_string_lossy(),
                            man.nations.map_or(0, |n| n.len())
                        );
                        success += 1;
                    }
                    Err(e) => println!(
                        "Error parsing {}: {}",
                        entry.file_name().to_string_lossy(),
                        e
                    ),
                }
            }
        }
    }
    println!("Total: {}, Success: {}", total, success);
}
