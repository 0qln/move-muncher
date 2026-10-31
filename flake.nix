{
  description = "move-muncher";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";

    nixpkgs.url = "github:NixOS/nixpkgs/master";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [];
      systems = ["x86_64-linux"];
      perSystem = {
        config,
        self',
        inputs',
        pkgs,
        system,
        ...
      }:
        with pkgs.lib; let
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
          # cargoLock = {
          #   allowBuiltinFetchGit = true;
          #   lockFile = ./Cargo.lock;
          # };
          # engineToml = builtins.fromTOML (builtins.readFile ./nephrid/Cargo.toml);
          # version = engineToml.package.version;
          # packageDerivation = features:
          #   rustPlatform.buildRustPackage {
          #     pname = packageName features;
          #     inherit version;
          #     src = ./.;
          #     inherit cargoLock;
          #     buildNoDefaultFeatures = true;
          #     buildFeatures = features;
          #     nativeBuildInputs = [pkgs.pkg-config];
          #     buildInputs =
          #       [pkgs.zlib]
          #       ++ (with pkgs-cuda; [
          #         cudaPackages_12_8.cuda_cudart
          #         cudaPackages_12_8.cudnn
          #         cudaPackages_12_8.cudatoolkit
          #       ]);
          #     meta.mainProgram = "nephrid";
          #     cargoBuildFlags = ["--bin" "nephrid"];
          #     cargoTestFlags = ["--bin" "nephrid"];
          #   };
        in {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [(import inputs.rust-overlay)];
          };

          packages = mkMerge [];

          devShells.default = pkgs.mkShell {
            packages = let
              # python packages for weather-factory-pkgs
              weather-factory-pkgs = ps: with ps; [matplotlib];
            in
              (with pkgs; [
                cutechess
                fastchess
                bacon
                perf
                act
                (python3.withPackages (ps: weather-factory-pkgs ps))
                gnuplot
                cargo-hack
              ])
              ++ (with pkgs-cuda; [
                # reference: https://discourse.nixos.org/t/cuda-12-8-support-in-nixpkgs/60645/39
                autoconf
                binutils
                curl
                freeglut
                git
                gitRepo
                gnumake
                gnupg
                gperf
                libGL
                libGLU
                linuxPackages.nvidia_x11
                m4
                ncurses5
                procps
                stdenv.cc
                unzip
                util-linux
                xorg.libX11
                xorg.libXext
                xorg.libXi
                xorg.libXmu
                xorg.libXrandr
                xorg.libXv
                zlib
              ]);

            buildInputs = [
              rustToolchain
            ];

            shellHook =
              # bash
              ''
                # Rust configuration
                export RUST_BACKTRACE=full
                export PROJECT_ROOT="$PWD"
              ''
              +
              # bash
              ''
                echo "yo! o/"
              ''
              +
              # bash
              ''
                # PATH
                export PATH="$PWD/benchmarking/bin:$PATH"
                export PATH="$PWD/bin:$PATH"
              '';
          };
        };
      flake = {};
    };
}
