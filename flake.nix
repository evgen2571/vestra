{
  description = "Video editor development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      softwareVulkan = pkgs.mkShell {
        packages = with pkgs; [
          cargo
          rustc
          rustfmt
          clippy
          pkg-config
          ffmpeg
          vulkan-loader
          vulkan-tools
          mesa
        ];

        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
          pkgs.vulkan-loader
          pkgs.mesa
        ];

        # Resolve the ICD through the flake input, rather than baking a Nix
        # store hash into the repository. Lavapipe is a real headless Vulkan
        # adapter used for strict WGPU verification inside containers.
        VK_DRIVER_FILES = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
        VK_ICD_FILENAMES = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
        VIDEO_EDITOR_WGPU_BACKEND = "vulkan";
        VIDEO_EDITOR_WGPU_FORCE_FALLBACK = "1";
        VIDEO_EDITOR_REQUIRE_WGPU = "1";
      };
    in
    {
      devShells.${system} = {
        default = softwareVulkan;
        software-vulkan = softwareVulkan;
      };
    };
}
