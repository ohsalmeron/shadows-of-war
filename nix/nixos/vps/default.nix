{
  config,
  lib,
  pkgs,
  self,
  ...
}:

{
  imports = [
    ./gce.nix
    ./networking.nix
    ./users.nix
    ./acme.nix
    ./nginx.nix
    ./valkey.nix
    ./sow-services.nix
    ./dirs.nix
  ];

  options.sow = {
    adminSshKeys = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default =
        let
          readKeys =
            path:
            lib.filter (k: k != "" && !lib.hasPrefix "#" k) (
              lib.splitString "\n" (lib.removeSuffix "\n" (builtins.readFile path))
            );
        in
        readKeys ./authorized_keys;
      description = "SSH public keys for the bizkit admin user.";
    };
    acmeEmail = lib.mkOption {
      type = lib.types.str;
      default = "admin@shadowsofwar.io";
      description = "Email for Let's Encrypt ACME registration.";
    };
  };

  config = {
    system.stateVersion = "24.11";

    nix.settings.experimental-features = [
      "nix-command"
      "flakes"
    ];
    nix.settings.trusted-users = [
      "root"
      "bizkit"
    ];

    # Packages referenced by systemd (store paths pinned to flake revision).
    environment.systemPackages = [
      self.packages.x86_64-linux.sow-server
      self.packages.x86_64-linux.sow-relay
    ];
  };
}
