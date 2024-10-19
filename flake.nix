{
  inputs = {
    cargo2nix.url = "github:cargo2nix/cargo2nix/release-0.11.0";
    flake-utils.follows = "cargo2nix/flake-utils";
    nixpkgs.follows = "cargo2nix/nixpkgs";
  };
  outputs = { nixpkgs, flake-utils, cargo2nix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ cargo2nix.overlays.default ];
        };
        rustPkgs = pkgs.rustBuilder.makePackageSet {
          rustVersion = "1.75.0";
          packageFun = import ./Cargo.nix;
          extraRustComponents = [ "clippy" ];
        };
      in
      {
        packages.default = rustPkgs.workspace.porc { };
        checks = {
          test = pkgs.rustBuilder.runTests rustPkgs.workspace.porc { };
        };
        devShells.default = pkgs.mkShell {
          buildInputs = [ pkgs.rust-analyzer ];
        };
        apps.generateCargoNix = {
          type = "app";
          program =
            pkgs.lib.getExe (pkgs.writeShellApplication
              {
                name = "generateCargoNix";
                runtimeInputs = [ cargo2nix.packages.${system}.default ];
                text = ''
                  cargo2nix . --overwrite --locked
                '';
              });
        };
      }
    );
}
