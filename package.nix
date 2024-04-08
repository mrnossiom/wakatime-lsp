{ lib, rustPlatform, gitignore }:

let
  inherit (gitignore.lib) gitignoreSource;

  src = gitignoreSource ./.;
  cargoTOML = lib.importTOML "${src}/Cargo.toml";
in
rustPlatform.buildRustPackage {
  pname = cargoTOML.package.name;
  version = cargoTOML.package.version;

  inherit src;

  cargoLock = { lockFile = "${src}/Cargo.lock"; };

  nativeBuildInputs = [ ];
  buildInputs = [ ];

  meta = {
    inherit (cargoTOML.package) description homepage license;
    maintainers = [ "mrnossiom" ];
    mainProgram = "wakatime-lsp";
  };
}
