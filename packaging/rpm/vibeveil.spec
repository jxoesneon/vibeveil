Name:           vibeveil
Version:        0.1.0
Release:        1%{?dist}
Summary:        Universal music-driven dynamic wallpaper & semantic desktop theming engine

License:        MIT or Apache-2.0
URL:            https://github.com/jxoesneon/vibeveil
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz

BuildRequires:  cargo >= 1.75
BuildRequires:  rust >= 1.75
BuildRequires:  systemd-rpm-macros
BuildRequires:  dbus-devel

Requires:       dbus
Recommends:     mpvpaper
Recommends:     swww
Recommends:     ffmpeg
Recommends:     rofi
Recommends:     wofi
Recommends:     hyprlock

%description
VibeVeil is an ultra-low-latency, zero-polling Linux desktop daemon written
in Rust that synchronizes your desktop wallpaper, Material You borders,
and lockscreen accents with your currently playing audio stream across
Spotify and all MPRIS-compliant media players.

%prep
%autosetup -p1

%build
cargo build --release --locked

# Generate shell completions
./target/release/vibeveil completions bash > vibeveil.bash
./target/release/vibeveil completions zsh > _vibeveil
./target/release/vibeveil completions fish > vibeveil.fish

%install
install -Dm755 target/release/vibeveil %{buildroot}%{_bindir}/vibeveil
install -Dm644 packaging/systemd/vibeveil.service %{buildroot}%{_userunitdir}/vibeveil.service
install -Dm644 vibeveil.bash %{buildroot}%{_datadir}/bash-completion/completions/vibeveil
install -Dm644 _vibeveil %{buildroot}%{_datadir}/zsh/site-functions/_vibeveil
install -Dm644 vibeveil.fish %{buildroot}%{_datadir}/fish/vendor_completions.d/vibeveil.fish

%check
cargo test --release --locked

%post
%systemd_user_post vibeveil.service

%preun
%systemd_user_preun vibeveil.service

%files
%license Cargo.toml
%doc README.md docs/
%{_bindir}/vibeveil
%{_userunitdir}/vibeveil.service
%{_datadir}/bash-completion/completions/vibeveil
%{_datadir}/zsh/site-functions/_vibeveil
%{_datadir}/fish/vendor_completions.d/vibeveil.fish

%changelog
* Tue Sep 15 2026 Jose Eduardo Rojas Jimenez <joseeduardox@gmail.com> - 0.1.0-1
- Initial release of VibeVeil daemon (0.1.0)
