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
    deployUser = lib.mkOption {
      type = lib.types.str;
      default = "sow";
      description = "Unix user for deploy, nginx static roots, and systemd services.";
    };
    dataRoot = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/sow";
      description = "Runtime data root (prod/ptr map libraries and server working dirs).";
    };
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
      description = "SSH public keys for the deploy admin user.";
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
      config.sow.deployUser
    ];

    # Packages referenced by systemd (store paths pinned to flake revision).
    environment.systemPackages = [
      self.packages.x86_64-linux.sow-server
      self.packages.x86_64-linux.sow-relay
    ];
  };
}
