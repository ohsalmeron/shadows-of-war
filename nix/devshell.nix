{
  pkgs,
  rustToolchain,
  system,
}:

let
  linuxGnuLinker =
    if pkgs.stdenv.isDarwin then
      pkgs.pkgsCross.gcc64.stdenv.cc + "/bin/x86_64-unknown-linux-gnu-gcc"
    else
      null;
in
pkgs.mkShell {
  packages =
    with pkgs;
    [
      rustToolchain
      binaryen
      libwebp
      brotli
      terser
      openssh
      rsync
      valkey
      pkg-config
      curl
      python3
    ]
    ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
      openssl
    ]
    ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
      pkgs.pkgsCross.gcc64
    ];

  shellHook = ''
    export SOW_IN_NIX_SHELL=1
    export SOW_CWEBP="${pkgs.libwebp}/bin/cwebp"
    export SOW_BROTLI="${pkgs.brotli}/bin/brotli"
    export SOW_WASM_OPT="${pkgs.binaryen}/bin/wasm-opt"
    export PATH="${pkgs.libwebp}/bin:${pkgs.brotli}/bin:${pkgs.terser}/bin:${pkgs.binaryen}/bin:''${HOME}/.cargo/bin:''${PATH}"
    ${pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
      export CARGO_TARGET_x86_64_unknown_linux_gnu_LINKER="${linuxGnuLinker}"
      export CC_x86_64_unknown_linux_gnu="${linuxGnuLinker}"
    ''}
    if ! command -v wasm-bindgen >/dev/null 2>&1 \
        || ! wasm-bindgen --version 2>/dev/null | grep -qF '0.2.122'; then
      echo "==> installing wasm-bindgen-cli 0.2.122 (must match workspace wasm-bindgen)..."
      cargo install wasm-bindgen-cli --version 0.2.122 --locked --force
    fi
    echo "sow nix dev shell — Rust $(rustc --version | cut -d' ' -f2) (${system})"
    echo "  ./sow infra       ./sow prod -v   ./sow ptr -v   ./sow cg   ./sow local"
  '';
}
