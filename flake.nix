{
  description = "Cherry-pick tool for Nixpkgs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      eachSystem = f: nixpkgs.lib.genAttrs supportedSystems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = eachSystem (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "qp";
          version = "0.2.0";

          src = ./.;

          cargoHash = "sha256-Pz5TiGFFRjbSDePnR67idflg+MRMMUEQywOaYoOADds=";
        };
      });
    };
}
