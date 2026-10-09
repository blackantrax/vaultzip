# Roadmap

## v0.1 (current)

- ZIP create, extract and list
- AES-256 encryption
- Command line interface
- Desktop app (egui)
- CI on Windows, macOS and Linux

## v0.2

- Desktop app: create, extract, drag and drop, strength meter, progress bar and cancel (done)
- Windows icon and version information (done)
- Explorer right-click menu through per-user registry entries (done)
- Windows 11 modern context menu through a packaged shell extension
- Windows installer (Inno Setup) with right-click menu, Open with entry and clean uninstall (done, tested in CI)
- Code signing for the installer and executables: wired into releases, switched on once the Azure setup in docs/SIGNING.md is done
- Wayland support on Linux
- Windows Explorer context menu integration
- Open and extract 7z, tar and gz archives

## v0.3

- Create 7z archives
- Encrypted file names
- Split archives and recovery data
- macOS notarization and a macOS installer

## Trust work (ongoing)

- Reproducible builds and signed releases with checksums
- Third-party code review and published report
- Fuzz testing of archive parsing
