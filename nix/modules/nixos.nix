packagesFile:

{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.libsnow;
  toml = builtins.fromTOML (builtins.readFile packagesFile);

  getPkg = name: lib.getAttrFromPath (lib.splitString "." name) pkgs;

  optionFragments =
    opts: lib.mapAttrsToList (path: value: lib.setAttrByPath (lib.splitString "." path) value) opts;

  systemPkgs = map getPkg (toml.system.packages or [ ]);
  systemOptFragments = optionFragments (toml.system.options or { });

  homeUsers = toml.home or { };

  configJson = builtins.toJSON (
    lib.filterAttrs (_: v: v != null) {
      systemconfig = cfg.systemconfig;
      homeconfig = cfg.homeconfig;
      flake = cfg.flake;
      host = cfg.host;
      generations = cfg.generations;
      mode = "toml";
      packages_file = cfg.packages_file;
    }
  );
in
{
  options.libsnow = {
    packages_file = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Runtime filesystem path to packages.toml.";
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
  };

  config = lib.mkMerge (
    [
      { environment.systemPackages = systemPkgs; }

      { environment.etc."libsnow/config.json".text = configJson; }

      {
        home-manager.users = lib.mapAttrs (
          _user: userCfg:
          lib.mkMerge (
            [
              { home.packages = map getPkg (userCfg.packages or [ ]); }
            ]
            ++ optionFragments (userCfg.options or { })
          )
        ) homeUsers;
      }
    ]
    ++ systemOptFragments
  );
}
