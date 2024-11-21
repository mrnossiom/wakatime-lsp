{ lib
, gitignore

, rustPlatform
, makeWrapper
, wakatime
}:

with lib;

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

  nativeBuildInputs = [ makeWrapper ];
  buildInputs = [ ];

  postFixup = ''
    wrapProgram $out/bin/wakatime-ls \
      --suffix PATH : ${makeBinPath [ wakatime ]}
  '';

  meta = {
    inherit (cargoTOML.package) description homepage license;
    maintainers = [ "mrnossiom" ];
    mainProgram = "wakatime-ls";
  };
}
