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
  userPkgs = (toml.home.${cfg.user}.packages or [ ]);
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

    user = lib.mkOption {
      type = lib.types.str;
      description = "Username to read home packages for from the TOML file.";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = map getPkg userPkgs;
  };
}
