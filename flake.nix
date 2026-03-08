{
  description = "kx-engine dev shell";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, fenix, ... }:
  let
    system = "x86_64-linux";
    pkgs = import nixpkgs { inherit system; };

    toolchain = fenix.packages.${system}.complete.withComponents [
      "cargo"
      "clippy"
      "rust-src"
      "rustc"
      "rustfmt"
    ];
  in
  {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = with pkgs; [
        toolchain
        fenix.packages.${system}.rust-analyzer

        # linker + build tools
        gcc
        pkg-config
        cmake

        # vulkan
        vulkan-loader
        vulkan-headers
        vulkan-validation-layers
        vulkan-tools
        shaderc

        # windowing (wayland + x11 for winit)
        wayland
        libxkbcommon
        libx11
        libxcursor
        libxrandr
        libxi
      ];

      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
        pkgs.vulkan-loader
        pkgs.wayland
        pkgs.libxkbcommon
      ];

      VULKAN_SDK = "${pkgs.vulkan-headers}";
      VK_LAYER_PATH = "${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d";
    };
  };
}
