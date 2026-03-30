{
  cargo,
  makeWrapper,
  openssl,
  pkg-config,
  rustc,
  rustPlatform,
  bash,
  desktop-file-utils,
  hicolor-icon-theme,
  shared-mime-info,
  gtk3,
  substitute,
}:

rustPlatform.buildRustPackage rec {
  pname = "libsnow-helper";
  version = "0.0.1";

  src = [ ../../libsnow-helper ];

  cargoLock.lockFile = ../../libsnow-helper/Cargo.lock;

  nativeBuildInputs = [
    cargo
    makeWrapper
    pkg-config
    rustc
    rustPlatform.cargoSetupHook
  ];

  buildInputs = [
    desktop-file-utils
    hicolor-icon-theme
    shared-mime-info
    gtk3
  ];

  postInstall = ''
    mv $out/bin $out/libexec

    install -Dm644 ../libsnow-helper/data/org.snowflakeos.LibSnow.Helper1.service.in \
      $out/share/dbus-1/system-services/org.snowflakeos.LibSnow.Helper1.service
    substituteInPlace $out/share/dbus-1/system-services/org.snowflakeos.LibSnow.Helper1.service \
      --replace-fail "@bindir@" "$out/libexec"

    install -Dm644 ../libsnow-helper/data/libsnow-helper.service.in \
      $out/lib/systemd/system/libsnow-helper.service
    substituteInPlace $out/lib/systemd/system/libsnow-helper.service \
      --replace-fail "@bindir@" "$out/libexec"

    install -Dm644 ../libsnow-helper/data/org.snowflakeos.LibSnow.Helper1.conf \
      $out/share/dbus-1/system.d/org.snowflakeos.LibSnow.Helper1.conf

    install -Dm644 ../libsnow-helper/data/org.snowflakeos.libsnow.policy \
      $out/share/polkit-1/actions/org.snowflakeos.libsnow.policy

    install -Dm644 ../libsnow-helper/data/org.snowflakeos.LibSnow.UserHelper1.service.in \
      $out/share/dbus-1/services/org.snowflakeos.LibSnow.UserHelper1.service
    substituteInPlace $out/share/dbus-1/services/org.snowflakeos.LibSnow.UserHelper1.service \
      --replace-fail "@bindir@" "$out/libexec"

    install -Dm755 ${./update-icons.trigger} $out/share/libsnow/triggers/update-icons.trigger
    substitute ${./update-icons.trigger} $out/share/libsnow/triggers/update-icons.trigger \
      --subst-var-by bash ${bash} \
      --subst-var-by desktop-file-utils ${desktop-file-utils} \
      --subst-var-by hicolor-icon-theme ${hicolor-icon-theme} \
      --subst-var-by shared-mime-info ${shared-mime-info} \
      --subst-var-by gtk3 ${gtk3}
  '';
}
