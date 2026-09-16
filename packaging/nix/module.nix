self: { config, lib, pkgs, ... }:

let
  cfg = config.services.vibeveil;
  tomlFormat = pkgs.formats.toml { };
in
{
  options.services.vibeveil = {
    enable = lib.mkEnableOption "VibeVeil music-driven dynamic wallpaper daemon";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.default;
      description = "The vibeveil package to use.";
    };

    settings = lib.mkOption {
      type = tomlFormat.type;
      default = { };
      description = "Configuration written to ~/.config/vibeveil/config.toml.";
      example = lib.literalExpression ''
        {
          general = {
            media_pool = "~/Pictures/Wallpapers";
            max_cache_mb = 250;
          };
          strategy = {
            mode = "hybrid";
            debounce_ms = 250;
          };
        }
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    systemd.user.services.vibeveil = {
      description = "VibeVeil Music-Driven Dynamic Wallpaper Daemon";
      after = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/vibeveil daemon";
        Restart = "always";
        RestartSec = "3s";
        Slice = "app.slice";

        # Sandboxing
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = "read-only";
        ReadWritePaths = [
          "%h/.cache/vibeveil"
          "%h/.config/vibeveil"
          "%h/.config/hypr"
          "%h/.config/noctalia"
        ];
      };
    };
  };
}
