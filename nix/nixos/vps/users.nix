{ config, ... }:

{
  users.users.bizkit = {
    isNormalUser = true;
    description = "Shadows of War deploy user";
    extraGroups = [ "wheel" ];
    openssh.authorizedKeys.keys = config.sow.adminSshKeys;
  };

  security.sudo.wheelNeedsPassword = false;
}
