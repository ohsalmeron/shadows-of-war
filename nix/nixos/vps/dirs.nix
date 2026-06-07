{ config, ... }:

let
  u = config.sow.deployUser;
  prod = "${config.sow.dataRoot}/prod";
  ptr = "${config.sow.dataRoot}/ptr";
in
{
  systemd.tmpfiles.rules = [
    "d /var/www/play.shadowsofwar.io/html 0755 ${u} users -"
    "d /var/www/ptr.shadowsofwar.io/html 0755 ${u} users -"
    "d /var/www/shadowsofwar.io/html 0755 ${u} users -"
    "d /var/www/shadowsofwar.io/html/assets 0755 ${u} users -"
    "d /var/www/shadowsofwar.io/html/assets/cdn 0755 ${u} users -"
    "d ${prod} 0755 ${u} users -"
    "d ${prod}/maps 0755 ${u} users -"
    "d ${ptr} 0755 ${u} users -"
    "d ${ptr}/maps 0755 ${u} users -"
  ];
}
