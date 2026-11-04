self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.flydigi-linux;
  rules = pkgs.writeTextFile {
    name = "flydigi-linux-udev-rules";
    # 必须排在 73-seat-late.rules 之前，uaccess 标签才会生效
    destination = "/lib/udev/rules.d/70-flydigi-linux.rules";
    text = ''
      # 飞智手柄的 vendor HID 接口（扳机命令）
      SUBSYSTEM=="hidraw", ATTRS{idVendor}=="37d7", TAG+="uaccess"
      # 全局 DS 模式的虚拟 DualSense
      KERNEL=="uhid", SUBSYSTEM=="misc", TAG+="uaccess"
    '';
  };
in
{
  options.programs.flydigi-linux = {
    enable = lib.mkEnableOption "FlydigiLinux（flydigid 服务与图形界面）";
    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "FlydigiLinux 包。";
    };
    wine = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      example = lib.literalExpression "pkgs.wineWow64Packages.stable";
      description = "运行原生 Windows 官方 mod（如 F1 系列）用的 wine；不设置则这类 mod 无法启动。";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    boot.kernelModules = [ "uhid" ];
    services.udev.packages = [ rules ];

    # XGameMonitor 要读游戏内存；ptrace_scope=1 时靠 CAP_SYS_PTRACE，
    # wrapper 把它作为 ambient capability 交给 flydigid，flydigid 再传给 mod 进程。
    security.wrappers.flydigid = {
      source = "${cfg.package}/bin/flydigid";
      capabilities = "cap_sys_ptrace+eip";
      owner = "root";
      group = "root";
    };

    systemd.user.services.flydigid = {
      description = "FlydigiLinux 服务（飞智扳机适配）";
      wantedBy = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      path = lib.optional (cfg.wine != null) cfg.wine;
      serviceConfig = {
        ExecStart = "${config.security.wrapperDir}/flydigid";
        Restart = "on-failure";
        RestartSec = 3;
      };
    };
  };
}
