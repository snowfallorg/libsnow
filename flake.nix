{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    { nixpkgs, ... }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems =
        f: nixpkgs.lib.genAttrs supportedSystems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      nixosModules.libsnow = import ./nix/modules/nixos.nix;
      homeModules.libsnow = import ./nix/modules/home.nix;

      packages = forAllSystems (pkgs: {
        libsnow-helper = pkgs.callPackage ./nix/packages/libsnow-helper.nix { };
      });

      overlays.default = _final: prev: {
        libsnow-helper = prev.callPackage ./nix/packages/libsnow-helper.nix { };
      };

      devShells = forAllSystems (pkgs: {
        default = pkgs.callPackage ./nix/shell.nix { };
      });
    };
}
