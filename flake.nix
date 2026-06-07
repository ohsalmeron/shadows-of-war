{
  description = "Shadows of War — devShell, server packages, NixOS VPS";

  nixConfig = {
    extra-experimental-features = [
      "nix-command"
      "flakes"
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    disko.url = "github:nix-community/disko";
    nixos-anywhere.url = "github:nix-community/nixos-anywhere";
    nixos-anywhere.inputs.nixpkgs.follows = "nixpkgs";
    nixos-anywhere.inputs.disko.follows = "disko";
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      disko,
      nixos-anywhere,
    }:
    let
      linuxPkgs = import nixpkgs {
        system = "x86_64-linux";
        overlays = [ (import rust-overlay) ];
      };
      linuxSrc = linuxPkgs.runCommand "shadows-of-war-src" {
        nativeBuildInputs = [ linuxPkgs.perl ];
      } ''
        cp -r ${self} $out
        chmod -R u+w $out
        perl -i -0pe 's/members = \[[^\]]*\]/members = ["sow-core", "sow-net", "sow-server", "sow-relay"]/s' $out/Cargo.toml
        sed -i '/^\[workspace\.dependencies\.blade-/d' $out/Cargo.toml
        sed -i '/^path = "blade\//d' $out/Cargo.toml
      '';
      sowPackagesLinux = import ./nix/packages.nix {
        pkgs = linuxPkgs;
        src = linuxSrc;
      };
    in
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" ];
          targets = [ "wasm32-unknown-unknown" "x86_64-unknown-linux-gnu" ];
        };
        devShell = import ./nix/devshell.nix {
          inherit pkgs rustToolchain system;
        };
      in
      {
        devShells.default = devShell;
        formatter = pkgs.nixfmt-rfc-style;

        packages =
          if system == "x86_64-linux" then
            sowPackagesLinux.packages
          else
            { };

        apps.nixos-anywhere = let
          pkg =
            nixos-anywhere.packages.${system}.nixos-anywhere
              or nixos-anywhere.packages.x86_64-linux.nixos-anywhere;
        in
        {
          type = "app";
          program = "${pkg}/bin/nixos-anywhere";
        };
      }
    )
    // {
      nixosConfigurations.vps = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit self inputs; };
        modules = [
          ./nix/nixos/vps
          ./nix/nixos/vps/filesystems.nix
        ];
      };

      nixosConfigurations.vps-install = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit self inputs; };
        modules = [
          disko.nixosModules.disko
          ./nix/nixos/vps
          ./nix/nixos/vps/disko-layout.nix
        ];
      };
    };
}
