// build.rs — link libfstack.a (whole-archive) + DPDK (pkg-config, order-preserved) + deps.
//
// DPDK static linking requires `--whole-archive` to wrap the driver archives:
// PMDs self-register via ELF constructors that nothing references by name, so a
// plain symbol-driven archive link silently drops them. libdpdk.pc already emits
// the flags in the right order (`-Wl,--whole-archive … -l:librte_*.a … -Wl,--no-whole-archive`),
// but the `pkg-config` crate re-emits `-l` and `-Wl` flags in two reordered groups,
// which strips the wrapper and kills every PMD constructor (TAP probe fails with
// "failed to initialize net_tap0 device" and no driver log). So parse pkg-config
// manually and pass each token as a raw link-arg, preserving the exact order.
//
// These flags apply to THIS crate's own targets (the examples). Binary crates
// that link the bridge (sow-relay) emit their own link args from their own
// build.rs so the archive order in the final binary matches the validated M3
// layout (constructor ordering in fstack's kernel emulation is order-sensitive).
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

            // Preserve DPDK constructor/init ELF sections (TLS/constructor-based PMD registration).
            println!("cargo:rustc-link-arg=-Wl,-z,nostart-stop-gc");
            println!("cargo:rustc-link-arg=-Wl,--export-dynamic");

            // F-Stack runtime deps (not covered by libdpdk.pc). Emitted after
            // libfstack.a so their symbols (RAND_bytes, inflateEnd, …) resolve.
            for lib in ["crypto", "z", "numa", "rt", "m", "dl"] {
                println!("cargo:rustc-link-arg=-l{lib}");
            }
            println!("cargo:rustc-link-arg=-pthread");
        }
        _ => {
            // No DPDK on the build host (dev/stub): emit nothing. The lib still
            // type-checks; only linking against real F-Stack requires DPDK.
            eprintln!("build.rs: pkg-config libdpdk not found; skipping DPDK link flags");
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
}
