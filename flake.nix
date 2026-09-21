{
  description = "doneyet — exits when it's done yet";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
            cargo-nextest
            cargo-deny
            just
            gh
            typos
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibsrc}";
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
