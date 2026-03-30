{
  cargo,
  openssl,
  pkg-config,
  rustc,
  rustPlatform,
  sqlite,
}:

rustPlatform.buildRustPackage {
  pname = "generate-db";
  version = "0.0.1";

  src = ../..;

  cargoLock = {
    lockFile = ../../Cargo.lock;
  };

  nativeBuildInputs = [
    cargo
    pkg-config
    rustc
    rustPlatform.cargoSetupHook
  ];

  buildInputs = [
    openssl
    sqlite
  ];

  cargoBuildFlags = [
    "--bin"
    "generate-db"
    "--features"
    "generate-db"
  ];
}
