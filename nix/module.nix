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
    dotnet = lib.mkOption {
      type = lib.types.package;
      default = pkgs.dotnet-runtime_10;
      defaultText = lib.literalExpression "pkgs.dotnet-runtime_10";
      description = "运行官方 .NET mod（ForzaDualSense、XGameMonitor）的 dotnet；net6 的 mod 通过 DOTNET_ROLL_FORWARD=Major 运行。";
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
    # security.wrappers 只接受 ELF，所以指向 wrapProgram 包装前的二进制，环境变量由服务提供。
    security.wrappers.flydigid = {
      source = "${cfg.package}/bin/.flydigid-wrapped";
      capabilities = "cap_sys_ptrace+eip";
      owner = "root";
      group = "root";
    };

    systemd.user.services.flydigid = {
      description = "FlydigiLinux 服务（飞智扳机适配）";
      wantedBy = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      path = [ cfg.dotnet ] ++ lib.optional (cfg.wine != null) cfg.wine;
      environment.FLYDIGI_XHELPER = "${cfg.package}/lib/flydigi-linux/libxhelper.so";
      serviceConfig = {
        ExecStart = "${config.security.wrapperDir}/flydigid";
        Restart = "on-failure";
        RestartSec = 3;
      };
    };
  };
}
