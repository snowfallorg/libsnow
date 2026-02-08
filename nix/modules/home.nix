configFile: user:

{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.libsnow;
  toml = builtins.fromTOML (builtins.readFile configFile);

  getPkg = name: lib.getAttrFromPath (lib.splitString "." name) pkgs;

  optionFragments =
    opts: lib.mapAttrsToList (path: value: lib.setAttrByPath (lib.splitString "." path) value) opts;

  userSection = toml.home.${user} or { };
  userPkgs = userSection.packages or [ ];
  userOptFragments = optionFragments (userSection.options or { });

  configJson = builtins.toJSON (
    lib.filterAttrs (_: v: v != null) {
      homeconfig = cfg.homeconfig;
      flake = cfg.flake;
      host = cfg.host;
      generations = cfg.generations;
      mode = "toml";
      config_file = cfg.config_file;
    }
  );
in
{
  options.libsnow = {
    config_file = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Path to the TOML config file.";
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
