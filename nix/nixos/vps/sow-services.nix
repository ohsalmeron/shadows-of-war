{ pkgs, self, ... }:

let
  sowServer = self.packages.x86_64-linux.sow-server;
  sowRelay = self.packages.x86_64-linux.sow-relay;

  prodEnv = {
    RUST_LOG = "info";
    SOW_REDIS_URL = "redis://127.0.0.1:6379";
    SOW_WS_LISTEN = "0.0.0.0:25565";
    SOW_MAPS_HTTP_LISTEN = "0.0.0.0:25566";
    SOW_MAPS_ROOT = "/home/bizkit/shadowsofwar/assets/maps";
    SOW_RELAY_BIN = "${sowRelay}/bin/sow-relay";
  };

  ptrEnv = {
    RUST_LOG = "info";
    SOW_REDIS_URL = "redis://127.0.0.1:6379";
    SOW_WS_LISTEN = "0.0.0.0:25575";
    SOW_MAPS_HTTP_LISTEN = "0.0.0.0:25576";
    SOW_MAPS_ROOT = "/home/bizkit/shadowsofwar-ptr/assets/maps";
    SOW_RELAY_BIN = "${sowRelay}/bin/sow-relay";
  };
in
{
  systemd.services.sow-server = {
    description = "Shadows of War Server (production)";
    wantedBy = [ "multi-user.target" ];
    after = [
      "network.target"
      "valkey.service"
    ];
    requires = [ "valkey.service" ];
    serviceConfig = {
      Type = "simple";
      User = "bizkit";
      Group = "users";
      WorkingDirectory = "/home/bizkit/shadowsofwar";
      ExecStart = "${sowServer}/bin/sow-server";
      Restart = "always";
      RestartSec = 3;
      KillMode = "process";
    };
    environment = prodEnv;
  };

  systemd.services.sow-server-ptr = {
    description = "Shadows of War Server (PTR)";
    wantedBy = [ "multi-user.target" ];
    after = [
      "network.target"
      "valkey.service"
    ];
    requires = [ "valkey.service" ];
    serviceConfig = {
      Type = "simple";
      User = "bizkit";
      Group = "users";
      WorkingDirectory = "/home/bizkit/shadowsofwar-ptr";
      ExecStart = "${sowServer}/bin/sow-server";
      Restart = "always";
      RestartSec = 3;
      KillMode = "process";
    };
    environment = ptrEnv;
  };
}
