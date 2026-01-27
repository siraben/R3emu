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
        r3emu = pkgs.rustPlatform.buildRustPackage {
          pname = "r3emu";
          version = "0.1.0";
          src = lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [
            pkgs.pkg-config
          ];
          buildInputs = [
            pkgs.SDL2
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          inputsFrom = [ r3emu ];
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rust-analyzer
            pkgs.clippy
          ];
        };

        packages.r3emu = r3emu;
        packages.default = r3emu;
      }
    );
}
