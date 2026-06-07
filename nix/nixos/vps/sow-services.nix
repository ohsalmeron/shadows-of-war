{ pkgs, self, config, ... }:

let
  sowServer = self.packages.x86_64-linux.sow-server;
  sowRelay = self.packages.x86_64-linux.sow-relay;

  prodData = "${config.sow.dataRoot}/prod";
  ptrData = "${config.sow.dataRoot}/ptr";

  prodEnv = {
    RUST_LOG = "info";
    SOW_REDIS_URL = "redis://127.0.0.1:6379";
    SOW_WS_LISTEN = "0.0.0.0:25565";
    SOW_MAPS_HTTP_LISTEN = "0.0.0.0:25566";
    SOW_MAPS_ROOT = "${prodData}/maps";
    SOW_RELAY_BIN = "${sowRelay}/bin/sow-relay";
  };

  ptrEnv = {
    RUST_LOG = "info";
    SOW_REDIS_URL = "redis://127.0.0.1:6379";
    SOW_WS_LISTEN = "0.0.0.0:25575";
    SOW_MAPS_HTTP_LISTEN = "0.0.0.0:25576";
    SOW_MAPS_ROOT = "${ptrData}/maps";
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
      User = config.sow.deployUser;
      Group = "users";
      WorkingDirectory = prodData;
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
      User = config.sow.deployUser;
      Group = "users";
      WorkingDirectory = ptrData;
      ExecStart = "${sowServer}/bin/sow-server";
      Restart = "always";
      RestartSec = 3;
      KillMode = "process";
    };
    environment = ptrEnv;
  };
}
