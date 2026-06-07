# GCE platform integration (metadata, clock, startup scripts).
{ config, pkgs, ... }:

{
  systemd.packages = [ pkgs.google-guest-agent ];
  systemd.services.google-guest-agent = {
    wantedBy = [ "multi-user.target" ];
    restartTriggers = [ config.environment.etc."default/instance_configs.cfg".source ];
  };
  systemd.services.google-startup-scripts.wantedBy = [ "multi-user.target" ];
  systemd.services.google-shutdown-scripts.wantedBy = [ "multi-user.target" ];

  environment.etc."default/instance_configs.cfg".text = ''
    [Accounts]
    useradd_cmd = useradd -m -s /run/current-system/sw/bin/bash -p * {user}

    [Daemons]
    accounts_daemon = false

    [InstanceSetup]
    set_host_keys = false

    [MetadataScripts]
    default_shell = ${pkgs.stdenv.shell}

    [NetworkInterfaces]
    setup = false
  '';

  boot.loader.grub = {
    enable = true;
    efiSupport = true;
    efiInstallAsRemovable = true;
    device = "nodev";
  };

  boot.initrd.availableKernelModules = [
    "virtio_pci"
    "virtio_scsi"
    "virtio_blk"
    "sd_mod"
    "ext4"
    "vfat"
  ];
  boot.initrd.kernelModules = [
    "virtio_pci"
    "virtio_scsi"
    "virtio_blk"
    "sd_mod"
  ];
  boot.initrd.supportedFilesystems = [
    "ext4"
    "vfat"
  ];
  boot.initrd.systemd.enable = true;
  boot.kernelModules = [
    "virtio_pci"
    "virtio_net"
  ];
  boot.kernelParams = [ "console=ttyS0" ];

  networking.extraHosts = ''
    169.254.169.254 metadata.google.internal metadata
  '';
  networking.interfaces.eth0.mtu = 1460;
}
