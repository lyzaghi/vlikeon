Name:           omniplay
Version:        1.0.0
Release:        1%{?dist}
Summary:        OmniPlay Gaming Runtime and Remote Play Client

License:        GPL-3.0-or-later AND MIT
URL:            https://vlikeon.vn
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  gcc-c++
BuildRequires:  alsa-lib-devel
BuildRequires:  pulseaudio-libs-devel
BuildRequires:  wayland-devel
BuildRequires:  libX11-devel
BuildRequires:  mesa-libEGL-devel
BuildRequires:  systemd-rpm-macros

Requires:       alsa-lib
Requires:       pulseaudio-libs
Requires:       wayland
Requires:       libX11
Requires:       mesa-libEGL

%description
OmniPlay is an open gaming runtime, emulation platform, and low-latency Remote
Play streaming client from Vlikeon.

%prep
%autosetup

%build
# Built in clean build root using Cargo

%install
rm -rf $RPM_BUILD_ROOT
mkdir -p $RPM_BUILD_ROOT%{_bindir}
mkdir -p $RPM_BUILD_ROOT%{_libexecdir}/omniplay
mkdir -p $RPM_BUILD_ROOT%{_userunitdir}
mkdir -p $RPM_BUILD_ROOT%{_datadir}/applications
mkdir -p $RPM_BUILD_ROOT%{_datadir}/icons/hicolor/256x256/apps

install -m 0755 bin/omniplay-launcher $RPM_BUILD_ROOT%{_bindir}/omniplay
install -m 0755 bin/omniplay-update $RPM_BUILD_ROOT%{_libexecdir}/omniplay/
install -m 0644 share/systemd/user/omniplay-native-host.service $RPM_BUILD_ROOT%{_userunitdir}/
install -m 0644 share/systemd/user/omniplay-native-host.socket $RPM_BUILD_ROOT%{_userunitdir}/
install -m 0644 share/applications/omniplay.desktop $RPM_BUILD_ROOT%{_datadir}/applications/

%post
%systemd_user_post omniplay-native-host.socket
/usr/bin/update-desktop-database &> /dev/null || :

%preun
%systemd_user_preun omniplay-native-host.socket omniplay-native-host.service

%postun
/usr/bin/update-desktop-database &> /dev/null || :

%files
%{_bindir}/omniplay
%{_libexecdir}/omniplay/
%{_userunitdir}/omniplay-native-host.*
%{_datadir}/applications/omniplay.desktop

%changelog
* Sun Oct 25 2026 Vlikeon Packaging Team <packaging@vlikeon.vn> - 1.0.0-1
- Initial release for OmniPlay 1.0.0
