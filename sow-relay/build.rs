// build.rs — replicate the M3 link order for the single F-Stack relay worker.
//
// fstack-bridge emits its link flags as `rustc-link-lib` (so they reach this
// dependent crate), but the proven M3 order (relay_full example) put every
// DPDK archive + libfstack.a as raw link-args AFTER the rlib inputs. Archive
// position in the link line determines constructor (`.init_array`) ordering in
// the final binary, and F-Stack's kernel emulation is sensitive to it: with a
// different order, `lo_set_defaultaddr` fails and `ff_veth_attach` segfaults
// in `uma_zalloc_pcpu_arg`. Emit the same args here so this binary links
// exactly like the validated example.
use std::process::Command;

fn main() {
    let fstack_lib_dir = std::env::var("FSTACK_LIB_DIR").unwrap_or_else(|_| {
        if std::path::Path::new("/usr/local/lib/libfstack.a").exists() {
            "/usr/local/lib".to_string()
        } else if std::path::Path::new("/home/azureuser/f-stack/lib/libfstack.a").exists() {
            "/home/azureuser/f-stack/lib".to_string()
        } else {
            "/opt/sow-dpdk/lib".to_string()
        }
    });
    println!("cargo:rerun-if-env-changed=FSTACK_LIB_DIR");
    let out = Command::new("pkg-config")
        .args(["--static", "--libs", "libdpdk"])
        .output();
    match out {
        Ok(out) if out.status.success() => {
            for tok in String::from_utf8_lossy(&out.stdout).split_whitespace() {
                println!("cargo:rustc-link-arg={}", tok);
            }

            println!("cargo:rustc-link-search=native={fstack_lib_dir}");

            // libfstack.a MUST be pulled in whole (fstack localizes all symbols
            // then re-exports only ff_api.symlist globals).
            println!("cargo:rustc-link-arg=-Wl,--whole-archive");
            println!("cargo:rustc-link-arg=-l:libfstack.a");
            println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");

            // Preserve DPDK constructor/init ELF sections.
            println!("cargo:rustc-link-arg=-Wl,-z,nostart-stop-gc");
            println!("cargo:rustc-link-arg=-Wl,--export-dynamic");

            // F-Stack runtime deps (resolved after libfstack.a).
            for lib in ["crypto", "z", "numa", "rt", "m", "dl"] {
                println!("cargo:rustc-link-arg=-l{lib}");
            }
            println!("cargo:rustc-link-arg=-pthread");
        }
        _ => {
            // Dev host without DPDK: nothing to link (cargo check still passes).
            eprintln!("build.rs: pkg-config libdpdk not found; skipping DPDK link flags");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
