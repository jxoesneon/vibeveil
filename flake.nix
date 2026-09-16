{
  description = "Universal music-driven dynamic wallpaper & semantic desktop theming engine in Rust";

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
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "vibeveil";
          version = "0.1.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [ pkgs.pkg-config pkgs.installShellFiles ];
          buildInputs = [ pkgs.dbus pkgs.openssl ];

          postInstall = ''
            install -Dm644 packaging/systemd/vibeveil.service $out/lib/systemd/user/vibeveil.service
            installShellCompletion --cmd vibeveil \
              --bash <($out/bin/vibeveil completions bash) \
              --zsh <($out/bin/vibeveil completions zsh) \
              --fish <($out/bin/vibeveil completions fish)
          '';

          meta = with pkgs.lib; {
            description = "Universal music-driven dynamic wallpaper & semantic desktop theming engine";
            homepage = "https://github.com/jxoesneon/vibeveil";
            license = with licenses; [ mit asl20 ];
            maintainers = [ ];
            mainProgram = "vibeveil";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
            pkg-config
            dbus
            openssl
          ];
        };
      }) // {
        nixosModules.default = import ./packaging/nix/module.nix self;
      };
}
