{
  inputs = {
    garnix-lib.url = "github:garnix-io/garnix-lib";
    rust-module.url = "github:garnix-io/rust-module";
  };
  outputs = inputs: inputs.garnix-lib.lib.mkModules {
    modules = [
      inputs.rust-module.garnixModules.default
    ];
    config = { pkgs, ... }: {
      rust.default = {
        src = ./.;
      };
    };
  };
}
