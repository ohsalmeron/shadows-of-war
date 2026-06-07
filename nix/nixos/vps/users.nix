{ config, ... }:

{
  users.users.${config.sow.deployUser} = {
    isNormalUser = true;
    description = "Shadows of War deploy user";
    extraGroups = [ "wheel" ];
    openssh.authorizedKeys.keys = config.sow.adminSshKeys;
  };

  security.sudo.wheelNeedsPassword = false;
}
