{
  mkShell,
  cargo,
  clippy,
  openssl,
  pkg-config,
  polkit,
  rust-analyzer,
  rustc,
  rustfmt,
  rustPlatform,
  sqlite,
}:

mkShell {
  nativeBuildInputs = [
    cargo
    clippy
    openssl
    pkg-config
    polkit
    rust-analyzer
    rustc
    rustfmt
    sqlite
  ];
  RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
}
