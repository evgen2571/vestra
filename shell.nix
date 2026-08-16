{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  packages = with pkgs; [
    cargo
    rustc
    rustfmt
    clippy
    pkg-config
    clang
    ffmpeg_8-headless
    vulkan-loader
    vulkan-tools
    mesa
  ];

  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.vulkan-loader pkgs.mesa ];
  VK_DRIVER_FILES = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
  VK_ICD_FILENAMES = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
  VESTRA_WGPU_BACKEND = "vulkan";
  VESTRA_WGPU_FORCE_FALLBACK = "1";
  VESTRA_REQUIRE_WGPU = "1";
}
