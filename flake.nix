{
  description = "Rust project Ankra";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = { self, nixpkgs }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs { inherit system; };
  in {
    # 1. The Package Output (This is what actually gets installed)
    packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
      pname = "ankra";
      version = "0.1.0"; # Match this to Cargo.toml version

      src = ./.;

      # Nix needs to hash dependencies to remain pure.
      # This requires a Cargo.lock file in your project root!
      cargoLock = {
        lockFile = ./Cargo.lock;

        outputHashes = {
            "zmerald-0.1.0" = "sha256-wC1c64mCFfwLtdDrmb43Dz36h1XcwLpHWMNTO6QDNjw=";
        };
      };
    };

    # 2. devShell (For local coding)
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = [
        pkgs.rustup
        pkgs.pkg-config
      ];

      shellHook = ''
        export CARGO_HOME=$PWD/.cargo
        export RUSTUP_HOME=$PWD/.rustup
        rustup default 1.88.0
        echo "Rust dev shell ready (1.88.0)"
      '';
    };
  };
}
