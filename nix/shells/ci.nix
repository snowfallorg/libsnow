{
  mkShell,
  awscli2,
  brotli,
  curl,
  generate-db,
}:

mkShell {
  nativeBuildInputs = [
    awscli2
    brotli
    curl
    generate-db
  ];
}
