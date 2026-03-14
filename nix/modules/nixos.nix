{
  config,
  lib,
  pkgs,
  libsnowSystemConfig,
  libsnowHomeConfig ? null,
  ...
}:

let
  cfg = config.libsnow;
  systemToml = builtins.fromTOML (builtins.readFile libsnowSystemConfig);
  homeToml =
    if libsnowHomeConfig != null then builtins.fromTOML (builtins.readFile libsnowHomeConfig) else { };

  getPkg = name: lib.getAttrFromPath (lib.splitString "." name) pkgs;

  optionFragments =
    opts: lib.mapAttrsToList (path: value: lib.setAttrByPath (lib.splitString "." path) value) opts;

  systemPkgs = map getPkg (systemToml.packages or [ ]);
  systemOptFragments = optionFragments (systemToml.options or { });

  configJson = builtins.toJSON (
    lib.filterAttrs (_: v: v != null) {
      inherit (cfg)
        systemconfig
        homeconfig
        flake
        host
        generations
        system_config_file
        home_config_file
        system_for_home_manager
        ;
      mode = "toml";
    }
  );
in
{
  options.libsnow = {
    system_config_file = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Path to the system TOML config file.";
    };

    home_config_file = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Path to the home-manager TOML config file.";
    };

    systemconfig = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Path to the NixOS configuration file.";
    };

    homeconfig = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Path to the home-manager configuration file.";
    };

    flake = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Path to the NixOS flake file.";
    };

    host = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = config.networking.hostName;
      description = "NixOS flake configuration name.";
    };

    generations = lib.mkOption {
      type = lib.types.nullOr lib.types.ints.unsigned;
      default = 5;
      description = "Number of NixOS generations to keep.";
    };

    system_for_home_manager = lib.mkOption {
      type = lib.types.nullOr lib.types.bool;
      default = null;
      description = "Whether home-manager is configured as part of the system config or seperately."; 
    };
  };

  config = lib.mkMerge (
    [
      { environment.systemPackages = systemPkgs; }

      { environment.etc."libsnow/config.json".text = configJson; }
    ]
    ++ lib.optionals (libsnowHomeConfig != null) [
      {
        home-manager.users = lib.mapAttrs (
          _user: userCfg:
          lib.mkMerge (
            [
              { home.packages = map getPkg (userCfg.packages or [ ]); }
            ]
            ++ optionFragments (userCfg.options or { })
          )
        ) homeToml;
      }
    ]
    ++ systemOptFragments
  );
}
