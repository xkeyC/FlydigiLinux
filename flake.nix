{
  description = "FlydigiLinux：Linux 版飞智空间站，复刻力回馈扳机适配（官方 mod）与 DS 模式";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: rec {
        flydigi-linux = pkgs.callPackage ./nix/package.nix { };
        default = flydigi-linux;
      });

      nixosModules.default = import ./nix/module.nix self;

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rustfmt clippy rust-analyzer dotnet-runtime_10 ];
          # eframe 运行时 dlopen 的图形与字体库
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (import ./nix/gui-libs.nix pkgs);
        };
      });
    };
}
