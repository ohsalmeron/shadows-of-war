# Static mounts for GCE (/dev/sda). Disko is only for nixos-anywhere first repartition.
{ ... }:

{
  fileSystems."/" = {
    device = "/dev/disk/by-uuid/bfcd9e2c-1155-4ce7-9619-e20d8052e8c4";
    fsType = "ext4";
    options = [
      "defaults"
      "noatime"
    ];
  };

  fileSystems."/boot" = {
    device = "/dev/disk/by-uuid/D733-6B44";
    fsType = "vfat";
    options = [
      "defaults"
      "noatime"
    ];
  };
}
