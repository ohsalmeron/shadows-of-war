//! Build identity: content-hash of the server's source trees (sow-server +
//! sow-core). Same sources → same epoch → byte-identical deploys stay
//! granular (no spurious restarts); ANY source change flips the epoch so the
//! running jail can prove which build it executes via [SERVER-BOOT].
use std::fs;
use std::path::Path;

fn fnv1a(bytes: &[u8], state: &mut u64) {
    for b in bytes {
        *state ^= *b as u64;
        *state = state.wrapping_mul(0x100000001b3);
    }
}

fn hash_rs_dir(dir: &Path, state: &mut u64) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        if path.is_dir() {
            if path.file_name().map_or(false, |n| n == "target") {
                continue;
            }
            hash_rs_dir(&path, state);
        } else if path.extension().map_or(false, |x| x == "rs") {
            fnv1a(path.to_string_lossy().as_bytes(), state);
            if let Ok(bytes) = fs::read(&path) {
                fnv1a(&bytes, state);
            }
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=../sow-core/src");
    let mut state: u64 = 0xcbf29ce484222325u64;
    hash_rs_dir(Path::new("src"), &mut state);
    hash_rs_dir(Path::new("../sow-core/src"), &mut state);
    let out = std::env::var("OUT_DIR").expect("OUT_DIR set by cargo");
    fs::write(
        format!("{out}/build_epoch.rs"),
        format!("pub const BUILD_EPOCH: u64 = {state:#018x};"),
    )
    .expect("write build_epoch.rs");
}
