{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.libsnow;
  toml = builtins.fromTOML (builtins.readFile cfg.packagesFile);
  getPkg = name: lib.getAttrFromPath (lib.splitString "." name) pkgs;
  systemPkgs = map getPkg (toml.system.packages or [ ]);
  homeUsers = toml.home or { };
in
{
  options.libsnow = {
    enable = lib.mkEnableOption "libsnow declarative package management" // {
      default = true;
    };

    packagesFile = lib.mkOption {
      type = lib.types.path;
      description = "Path to the libsnow packages.toml file.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = systemPkgs;

    home-manager.users = lib.mapAttrs (_user: userCfg: {
      home.packages = map getPkg (userCfg.packages or [ ]);
    }) homeUsers;
  };
}
