{
  description = "ankra";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
  };

  outputs = { self, nixpkgs, fenix }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };

      toolchain = fenix.packages.${system}.toolchainOf {
        channel = "stable";
        date = "2023-01-01"; # start here, adjust if needed
      };
    in {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = [
          toolchain

          pkgs.pkg-config
          pkgs.wayland
          pkgs.libxkbcommon
        ];
      };
    };
}
