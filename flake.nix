{
  description = "R3emu development flake";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;

        r3emu = pkgs.stdenv.mkDerivation {
          pname = "r3emu";
          version = "0.1";
          src = lib.cleanSource ./.;
          strictDeps = true;
          nativeBuildInputs = [
            pkgs.meson
            pkgs.ninja
            pkgs.pkg-config
          ];
          buildInputs = [
            pkgs.SDL2
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          inputsFrom = [ r3emu ];
          packages = [
            pkgs.clang
            pkgs.gcc
            pkgs.gnumake
          ];
        };

        packages.r3emu = r3emu;
        packages.default = r3emu;
      }
    );
}
