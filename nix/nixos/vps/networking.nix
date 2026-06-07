{ config, ... }:

{
  networking.hostName = "sow-vps";
  networking.domain = "shadowsofwar.io";

  networking.firewall = {
    enable = true;
    allowedTCPPorts = [
      22
      80
      443
    ];
  };

  services.openssh = {
    enable = true;
    settings = {
      PasswordAuthentication = false;
      PermitRootLogin = "prohibit-password";
    };
  };

  time.timeZone = "UTC";
}
