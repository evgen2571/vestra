{
  description = "Vestra development environments";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = { nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = function: nixpkgs.lib.genAttrs systems (system: function system);
    in {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          inherit (pkgs) lib;
          ffmpeg = pkgs.ffmpeg_8-headless;
          ffmpegDev = ffmpeg.dev;
          ffmpegLib = ffmpeg.lib;

          nativePackages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            python313
            uv
            pkg-config
            clang
            gnumake
            ffmpeg
          ];

          mkVestraShell = { softwareWgpu ? false }:
            pkgs.mkShell {
              packages = nativePackages
                ++ lib.optionals softwareWgpu [
                  pkgs.mesa
                  pkgs.vulkan-loader
                  pkgs.vulkan-tools
                ];
              buildInputs = [ ffmpegDev ffmpegLib ];

              # ffmpeg-sys-next needs the development output, while runtime
              # commands use the package's executable output above.
              PKG_CONFIG_PATH = lib.makeSearchPath "lib/pkgconfig" [ ffmpegDev ];
              LD_LIBRARY_PATH = lib.makeLibraryPath [ ffmpegLib ];

              shellHook = lib.optionalString softwareWgpu ''
                export LD_LIBRARY_PATH="${lib.makeLibraryPath [ pkgs.vulkan-loader pkgs.mesa ]}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
                export VK_ICD_FILENAMES="${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json"
                export VESTRA_WGPU_BACKEND=vulkan
                export VESTRA_WGPU_FORCE_FALLBACK=1
                export VESTRA_REQUIRE_WGPU=1
                unset VESTRA_REQUIRE_HARDWARE_WGPU
              '';
            };
        in {
          default = mkVestraShell {};
          ci = mkVestraShell {};
          wgpu-software = mkVestraShell { softwareWgpu = true; };
        });
    };
}
