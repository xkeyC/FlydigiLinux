# FlydigiLinux

[中文](README.md) | English

Flydigi Space Station for Linux. The current goal is the **force-feedback trigger** features: fetch Flydigi's official list of supported games; download, install, enable and uninstall the official mods; and drive the APEX 5 triggers while a game runs. A global DS mode serves as a fallback for games without an official mod. Other features (button mapping, lighting, the screen and so on) have placeholders in the UI and will come later.

The guiding rule is to **run Flydigi's original mods wherever possible**. This project only replaces the service side of Space Station (SpaceStationService).

## Components

| crate | Role |
|---|---|
| `flydigid` | Background service: receives trigger commands from official mods on UDP 7878 and writes them to the controller; detects supported games and starts/stops the official mods or the vibration bind; runs the global DS mode; serves the GUI over a Unix socket |
| `flydigi-gui` | egui interface in Chinese and English (follows the system language; can be switched on the Service page) |
| `flydigi-core` | Official game list, mod install/uninstall, DSX command parsing, DualSense output report translation, IPC protocol |
| `flydigi-proto` | The controller's vendor HID protocol (usage page 0xFFA0, reports 3/4, `5A A5 <cmd>`) |
| `xhelper-shim` | A Linux implementation of `XHelper.dll`, which the official XGameMonitor depends on (`/proc/<pid>/mem`) |

## How each kind of supported game runs

| Official route | Examples | On Linux |
|---|---|---|
| Vibration bind | Apex Legends, Wuthering Waves, BF6 | flydigid sends command 82 when the game starts, so the triggers follow grip rumble |
| External mod: ForzaDualSense | Forza Horizon 4/5/6, Forza Motorsport | The original .NET program runs on Linux `dotnet` (`DOTNET_ROLL_FORWARD=Major`); enable Data Out in game to 127.0.0.1:5300 |
| External mod: XGameMonitor | Elden Ring, Cyberpunk 2077, Red Dead Redemption 2, The Witcher 3 | The original .NET program runs on `dotnet` with `XHelper.dll.so` (this project's shim) next to it, reading game memory as the official JSON configs describe |
| External mod: native Windows program | F1 23/24/25 | Runs in Wine (separate prefix, UDP only) |
| In-game mod | GTA V, Resident Evil 2/3/7, Fallout 4 | Extracted into the game folder (found by Steam appid, original files backed up); sends commands to UDP 7878 from inside the game |
| DS mode only | Horizon Forbidden West, God of War, etc. | Turn on the global DS mode |

## Global DS mode

Modelled on [ApexSenseBridge](https://github.com/ReynArts/ApexSenseBridge):

- A virtual USB DualSense is created through `/dev/uhid` (descriptor and feature reports taken from real hardware).
- Buttons and sticks come from xpad's evdev node, so remaps in the controller's onboard profile still apply; the node is grabbed with `EVIOCGRAB`. Motion and the PS button come from the vendor HID raw input stream (command 0x11 enables rawData, reports arrive as 0xEF); the original setting is restored on exit.
- Adaptive trigger effects from the game are translated to Flydigi trigger commands with ASB's rules (including the bow effect, which switches by trigger position); rumble goes to the grip motors. Strength and gain are adjustable.
- Games need Steam Input disabled and these launch options: `SDL_GAMECONTROLLER_IGNORE_DEVICES=0x37d7/0x2501 SDL_JOYSTICK_IGNORE_DEVICES=0x37d7/0x2501 %command%`.
- Audio haptics, touchpad swipes and the lightbar are not supported yet.

## Installation (NixOS)

```nix
{
  inputs.flydigi-linux.url = "github:xkeyC/FlydigiLinux";
  # …
  imports = [ inputs.flydigi-linux.nixosModules.default ];
  programs.flydigi-linux = {
    enable = true;
    wine = pkgs.wineWow64Packages.stable; # optional, needed for native Windows mods such as F1
  };
}
```

The module installs the package; adds udev rules giving the logged-in user access to the vendor hidraw node and `/dev/uhid`; loads `uhid`; grants `flydigid` `cap_sys_ptrace` through `security.wrappers` (XGameMonitor reads game memory, which needs it under `ptrace_scope=1`); and registers the `flydigid` user service (`systemctl --user`).

## Development

```sh
nix develop
cargo build
FLYDIGI_XHELPER=$PWD/target/debug/libxhelper.so cargo run -p flydigid
cargo run -p flydigi-gui
```

Data locations: the game list and mods are in `~/.local/share/flydigi-linux/`, state is in `~/.config/flydigi-linux/state.json`, the GUI language is in `~/.config/flydigi-linux/gui.json`, and cached images are in `~/.cache/flydigi-linux/images/`.

## Credits and license

- [ApexSenseBridge](https://github.com/ReynArts/ApexSenseBridge) (GPL-3.0-or-later): the overall DS mode design, adaptive trigger translation, and raw input stream decoding.
- [OpenFlydigi](https://github.com/mkaliaha/openflydigi) (MIT): protocol documentation, the DualSense descriptor and feature reports, and uhid usage.
- Flydigi's game list, mods and configs belong to Flydigi. This project downloads them from the official endpoints at runtime and does not redistribute them.

This project is released under GPL-3.0-or-later.
