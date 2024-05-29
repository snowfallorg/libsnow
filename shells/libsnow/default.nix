{ mkShell
, cargo
, clippy
, openssl
, pkg-config
, polkit
, rust
, rust-analyzer
, rustc
, rustfmt
, sqlite
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
  RUST_SRC_PATH = "${rust.packages.stable.rustPlatform.rustLibSrc}";
}
