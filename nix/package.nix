{
  lib,
  pkgs,
  rustPlatform,
  makeWrapper,
  makeDesktopItem,
  copyDesktopItems,
  dotnet-runtime_10,
}:

rustPlatform.buildRustPackage {
  pname = "flydigi-linux";
  version = "0.1.0";

  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type: !(lib.hasPrefix "result" (baseNameOf path)) && baseNameOf path != "target";
  };
  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [
    makeWrapper
    copyDesktopItems
  ];

  desktopItems = [
    (makeDesktopItem {
      name = "flydigi-linux";
      desktopName = "Flydigi Space Station for Linux";
      genericName = "Flydigi Controller Settings";
      comment = "Force-feedback triggers (official Flydigi mods) and DS mode for Flydigi controllers";
      exec = "flydigi-gui";
      icon = "input-gaming";
      categories = [ "Game" ];
      keywords = [
        "Flydigi"
        "APEX"
        "gamepad"
        "controller"
        "trigger"
        "DualSense"
        "飞智"
        "空间站"
        "手柄"
        "扳机"
      ];
      startupWMClass = "flydigi-linux";
      extraConfig = {
        "Name[zh_CN]" = "飞智空间站 Linux";
        "GenericName[zh_CN]" = "飞智手柄设置";
        "Comment[zh_CN]" = "飞智手柄力回馈扳机适配（官方 mod）与 DS 模式";
      };
    })
  ];

  postInstall = ''
    # XGameMonitor 的 XHelper.dll 的 Linux 实现，flydigid 运行时链接到 mod 目录
    install -Dm755 target/*/release/libxhelper.so $out/lib/flydigi-linux/libxhelper.so 2>/dev/null \
      || install -Dm755 target/release/libxhelper.so $out/lib/flydigi-linux/libxhelper.so
    rm -f $out/lib/libxhelper.so

    # 官方 .NET mod 是 net6，用 DOTNET_ROLL_FORWARD=Major 跑在 .NET 10 上
    wrapProgram $out/bin/flydigid \
      --prefix PATH : ${lib.makeBinPath [ dotnet-runtime_10 ]} \
      --set-default FLYDIGI_XHELPER $out/lib/flydigi-linux/libxhelper.so
    wrapProgram $out/bin/flydigi-gui \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath (import ./gui-libs.nix pkgs)}
  '';

  meta = {
    description = "Linux 版飞智空间站：力回馈扳机适配（官方 mod）与 DS 模式";
    license = lib.licenses.gpl3Plus;
    platforms = [ "x86_64-linux" ];
    mainProgram = "flydigi-gui";
  };
}
