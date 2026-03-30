{ self }:
{
  config,
  lib,
  pkgs,
  libsnowHomeConfig,
  libsnowUser,
  ...
}:

let
  cfg = config.libsnow;
  toml = builtins.fromTOML (builtins.readFile libsnowHomeConfig);

  getPkg = name: lib.getAttrFromPath (lib.splitString "." name) pkgs;

  optionFragments =
    opts: lib.mapAttrsToList (path: value: lib.setAttrByPath (lib.splitString "." path) value) opts;

  userSection = toml.${libsnowUser} or { };
  userPkgs = userSection.packages or [ ];
  userOptFragments = optionFragments (userSection.options or { });

  configJson = builtins.toJSON (
    lib.filterAttrs (_: v: v != null) {
      inherit (cfg)
        home_config_file
        flake
        host
        generations
        ;
      mode = "toml";
    }
  );
in
{
  options.libsnow = {
    home_config_file = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Path to the home-manager TOML config file.";
    };

    flake = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Path to the flake file.";
    };

    host = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Flake configuration name. Defaults to hostname if unset.";
    };

    generations = lib.mkOption {
      type = lib.types.nullOr lib.types.ints.unsigned;
      default = 5;
      description = "Number of generations to keep.";
    };

    helper = {
      enable = lib.mkEnableOption "libsnow-helper session D-Bus service for home-manager operations" // {
        default = true;
      };

      package = lib.mkOption {
        type = lib.types.package;
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.libsnow-helper;
        description = "The libsnow-helper package to use.";
      };
    };
  };

  config = lib.mkMerge (
    [
      { home.packages = map getPkg userPkgs; }

      { xdg.configFile."libsnow/config.json".text = configJson; }

      (lib.mkIf cfg.helper.enable {
        xdg.dataFile."dbus-1/services/org.snowflakeos.LibSnow.UserHelper1.service".source =
          "${cfg.helper.package}/share/dbus-1/services/org.snowflakeos.LibSnow.UserHelper1.service";
      })
    ]
    ++ userOptFragments
  );
}
