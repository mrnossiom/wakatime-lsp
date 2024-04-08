{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    gitignore = {
      url = "github:hercules-ci/gitignore.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, gitignore }:
    let
      inherit (nixpkgs.lib) genAttrs;

      forAllSystems = genAttrs [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
      forAllPkgs = function: forAllSystems (system: function pkgs.${system});

      mkApp = (program: { type = "app"; inherit program; });

      pkgs = forAllSystems (system: (import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      }));
    in
    {
      formatter = forAllPkgs (pkgs: pkgs.nixpkgs-fmt);

      packages = forAllPkgs (pkgs: rec {
        default = wakatime-lsp;
        wakatime-lsp = pkgs.callPackage ./package.nix { inherit gitignore; };
      });
      apps = forAllSystems (system: rec {
        default = wakatime-lsp;
        wakatime-lsp = mkApp (pkgs.getExe self.packages.${system}.app);
      });

      devShells = forAllPkgs (pkgs:
        with pkgs.lib;
        let
          file-rust-toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rust-toolchain = file-rust-toolchain.override { extensions = [ "rust-analyzer" ]; };
        in
        {
          default = pkgs.mkShell rec {
            nativeBuildInputs = with pkgs; [
              pkg-config
              rust-toolchain
              act

              wakatime
            ];
            buildInputs = with pkgs; [ ];

            RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
            LD_LIBRARY_PATH = makeLibraryPath buildInputs;

            RUST_LOG = "info";
          };
        });
    };
}
