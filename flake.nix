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
      rec {
        packages = {
          porc = rustPkgs.workspace.porc { };
          default = packages.porc;
          generateCargoNix = pkgs.writeShellApplication
            {
              name = "generateCargoNix";
              runtimeInputs = [ cargo2nix.packages.${system}.default ];
              text = ''
                cargo2nix . --overwrite --locked
              '';
            };
        };
        checks = {
          test = pkgs.rustBuilder.runTests rustPkgs.workspace.porc {
            testCommand = bin:
              ''
                export INSTA_WORKSPACE_ROOT=${./.}
                ${bin}
              '';
          };
        };
        devShells.default = pkgs.mkShell {
          buildInputs = [ pkgs.rust-analyzer pkgs.cargo-insta ];
        };
        apps.generateCargoNix = {
          type = "app";
          program = pkgs.lib.getExe packages.generateCargoNix;
        };
      }
    );
}
