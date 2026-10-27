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
      mkQuickpick =
        pkgs:
        pkgs.rustPlatform.buildRustPackage {
          pname = "qp";
          version = "0.2.0";
          src = ./.;

          cargoHash = "sha256-Pz5TiGFFRjbSDePnR67idflg+MRMMUEQywOaYoOADds=";

          meta = {
            description = "Cherry-pick tool for Nixpkgs";
            mainProgram = "qp";
            license = with pkgs.lib.licenses; [ mit ];
            platforms = supportedSystems;
          };
        };
    in
    {
      devShells = eachSystem (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
          ];
        };
      });
      packages = eachSystem (pkgs: {
        default = mkQuickpick pkgs;
        quickpick = mkQuickpick pkgs;
      });

      apps = eachSystem (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.quickpick}/bin/qp";
        };
      });

      nixosModules.default = import ./modules/nixos.nix { inherit self; };
      homeManagerModules.default = import ./modules/home-manager.nix { inherit self; };
    };
}
