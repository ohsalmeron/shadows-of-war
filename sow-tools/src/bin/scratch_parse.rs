fn main() {
    let mut total = 0;
    let mut success = 0;
    for entry in std::fs::read_dir("assets/maps").unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            let map_path = entry.path().join("map.bin");
            if !map_path.exists() {
                continue;
            }
            total += 1;
            let map_key = entry.file_name().to_string_lossy().to_string();
            match std::fs::read(&map_path) {
                Ok(bytes) => match sow_core::map_file::parse(&bytes) {
                    Ok(map_file) => {
                        println!(
                            "OK {}: {} ({}x{}, {} spawns)",
                            map_key,
                            map_file.display_name,
                            map_file.width,
                            map_file.height,
                            map_file.spawns.len()
                        );
                        success += 1;
                    }
                    Err(e) => println!("Parse error {}: {}", map_key, e),
                },
                Err(e) => println!("Read error {}: {}", map_key, e),
            }
        }
    }
    println!("Total: {}, Success: {}", total, success);
}
