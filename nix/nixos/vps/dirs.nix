{ ... }:

{
  systemd.tmpfiles.rules = [
    "d /var/www/play.shadowsofwar.io/html 0755 bizkit users -"
    "d /var/www/ptr.shadowsofwar.io/html 0755 bizkit users -"
    "d /var/www/shadowsofwar.io/html 0755 bizkit users -"
    "d /var/www/shadowsofwar.io/html/assets 0755 bizkit users -"
    "d /var/www/shadowsofwar.io/html/assets/cdn 0755 bizkit users -"
    "d /home/bizkit/shadowsofwar 0755 bizkit users -"
    "d /home/bizkit/shadowsofwar/assets/maps 0755 bizkit users -"
    "d /home/bizkit/shadowsofwar-ptr/assets/maps 0755 bizkit users -"
    "d /home/bizkit/shadowsofwar-ptr 0755 bizkit users -"
  ];
}
