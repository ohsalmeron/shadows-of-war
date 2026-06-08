{
  pkgs,
  rustToolchain,
  system,
}:

let
  linuxGnuLinker =
    if pkgs.stdenv.isDarwin then
      pkgs.pkgsCross.gnu64.stdenv.cc + "/bin/x86_64-unknown-linux-gnu-gcc"
    else
      null;
in
pkgs.mkShell {
  packages =
    with pkgs;
    [
      rustToolchain
      binaryen
      openssh
      rsync
      valkey
      pkg-config
      curl
      python3
    ]
    ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
      openssl
    ];

  shellHook = ''
    export SOW_IN_NIX_SHELL=1
    export SOW_WASM_OPT="${pkgs.binaryen}/bin/wasm-opt"
    export PATH="${pkgs.binaryen}/bin:''${PATH}"
    ${pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
      export CARGO_TARGET_x86_64_unknown_linux_gnu_LINKER="${linuxGnuLinker}"
      export CC_x86_64_unknown_linux_gnu="${linuxGnuLinker}"
    ''}
    echo "sow nix dev shell — Rust $(rustc --version | cut -d' ' -f2) (${system})"
    echo "  ./sow infra       ./sow prod -v   ./sow ptr -v   ./sow cg   ./sow local"
  '';
}
