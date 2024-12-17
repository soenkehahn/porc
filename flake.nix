{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };
  outputs = { nixpkgs, flake-utils, crane, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        lib = nixpkgs.lib;
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.mkLib pkgs;
        commonArgs =
          let
            inner = {
              src = lib.fileset.toSource {
                root = ./.;
                fileset = lib.fileset.unions [
                  ./Cargo.toml
                  ./Cargo.lock
                  ./rust-toolchain
                  ./src
                ];
              };
              strictDeps = true;
            };
          in
          inner // {
            cargoArtifacts = craneLib.buildDepsOnly inner;
          };
      in
      rec {
        packages = {
          porc = craneLib.buildPackage (commonArgs // {
            doCheck = false;
          });
          default = packages.porc;
        };
        checks = {
          tests = craneLib.cargoTest commonArgs;
          clippy = craneLib.cargoClippy (commonArgs // {
            cargoClippyExtraArgs = "-- -Dwarnings";
          });
        };
        devShells.default = craneLib.devShell {
          packages = [ pkgs.rust-analyzer ];
        };
      }
    );
}
