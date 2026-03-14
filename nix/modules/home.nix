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
        homeconfig
        flake
        host
        generations
        ;
      home_config_file = cfg.home_config_file;
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

    homeconfig = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Path to the home-manager configuration file.";
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
  };

  config = lib.mkMerge (
    [
      { home.packages = map getPkg userPkgs; }

      { xdg.configFile."libsnow/config.json".text = configJson; }
    ]
    ++ userOptFragments
  );
}
