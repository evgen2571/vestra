{
  description = "Video editor development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          cargo
          rustc
          rustfmt
          clippy
          pkg-config
          vulkan-loader
          vulkan-tools
          ffmpeg
          python3
        ];

        LD_LIBRARY_PATH =
          pkgs.lib.makeLibraryPath [
            pkgs.vulkan-loader
          ]
          + ":/run/opengl-driver/lib";
      };
    };
}
