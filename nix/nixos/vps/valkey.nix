{ pkgs, ... }:

{
  systemd.services.valkey = {
    description = "Valkey (relay port registry)";
    wantedBy = [ "multi-user.target" ];
    after = [ "network.target" ];
    serviceConfig = {
      Type = "simple";
      ExecStart = "${pkgs.valkey}/bin/valkey-server --port 6379 --bind 127.0.0.1 --save \"\" --appendonly no";
      Restart = "always";
      RestartSec = 2;
    };
  };

  systemd.services.sow-valkey = {
    description = "Alias for valkey (legacy unit name)";
    wantedBy = [ "multi-user.target" ];
    before = [
      "sow-server.service"
      "sow-server-ptr.service"
    ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      ExecStart = "${pkgs.coreutils}/bin/true";
    };
  };
}
