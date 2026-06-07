# GCE disk layout for nixos-anywhere first install only (.#vps-install).
{
  disko.devices.disk.main = {
    device = "/dev/sda";
    type = "disk";
    content = {
      type = "gpt";
      partitions = {
        boot = {
          type = "EF00";
          size = "512M";
          priority = 1;
          content = {
            type = "filesystem";
            format = "vfat";
            mountpoint = "/boot";
            mountOptions = [
              "defaults"
              "noatime"
            ];
          };
        };
        root = {
          size = "100%";
          priority = 2;
          content = {
            type = "filesystem";
            format = "ext4";
            mountpoint = "/";
            mountOptions = [
              "defaults"
              "noatime"
            ];
          };
        };
      };
    };
  };
}
