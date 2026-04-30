{
  description = "Rust dev shell for Ankra";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.05";

  outputs = { self, nixpkgs }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs { inherit system; };
  in {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = [
        pkgs.rustup
        pkgs.pkg-config
      ];

      shellHook = ''
        export CARGO_HOME=$PWD/.cargo
        export RUSTUP_HOME=$PWD/.rustup
        rustup default 1.72.0
        echo "Rust dev shell ready (1.72.0)"
      '';
    };
  };
}
