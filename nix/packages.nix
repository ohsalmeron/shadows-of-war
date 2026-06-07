{
  pkgs,
  src,
}:

let
  commonArgs = {
    inherit src;
    cargoLock = {
      lockFile = "${src}/Cargo.lock";
      outputHashes = { };
    };
    nativeBuildInputs = with pkgs; [
      pkg-config
      openssl
    ];
    buildInputs = with pkgs; [ openssl ];
    doCheck = false;
  };

  mkSowBin = package:
    pkgs.rustPlatform.buildRustPackage (
      commonArgs
      // {
        pname = package;
        version = "0.31.0-beta.2";
        cargoBuildFlags = [ "-p ${package}" ];
      }
    );

  sow-server = mkSowBin "sow-server";
  sow-relay = mkSowBin "sow-relay";
in
{
  inherit sow-server sow-relay;

  packages = {
    inherit sow-server sow-relay;
  };
}
