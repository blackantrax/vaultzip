# Releasing VaultZip

## Publishing a release

1. Update the version in the root Cargo.toml and commit.
2. Create and push a tag that starts with v, for example v0.1.0.
3. The Release workflow builds every platform and attaches the files to a GitHub release:
   - VaultZip-Setup-x.y.z.exe: the Windows installer
   - vaultzip-x86_64-pc-windows-msvc.exe: the portable Windows desktop app
   - vaultzip-cli-x86_64-pc-windows-msvc.exe: the Windows command line tool
   - the macOS and Linux builds, and a SHA256SUMS file for each platform

On Windows the workflow also installs the installer silently on the runner, checks the files and registry entries, uninstalls it, and checks that everything was removed. A release is not published if that test fails.

## Testing the installer locally

On a Windows machine with Rust and Inno Setup 6:

    cargo build --release
    .\installer\build-installer.ps1 -Version 0.1.0 -BuildDir target\release
    .\installer\smoke-test.ps1 -Installer dist\VaultZip-Setup-0.1.0.exe

## Installer notes

- The AppId in installer/vaultzip.iss identifies the product to Windows. Never change it after the first public release, or upgrades will install side by side.
- The installer is per-user (no administrator rights). Right-click menu entries are written by the program itself (vaultzip-gui.exe --install-shell), so the installer and the in-app Settings page always create identical entries.
- Windows 11 shows the entries under Show more options. Showing them in the main menu requires a packaged shell extension, which is on the roadmap.

## Code signing

Unsigned installers trigger a Windows SmartScreen warning ("Windows protected your PC") until the file builds reputation. For a security product this is the most important trust gap to close before promoting the project. Options, from simplest to strongest:

1. Azure Trusted Signing: low cost, no hardware token, works from GitHub Actions.
2. An OV code signing certificate: works, but reputation builds slowly.
3. An EV code signing certificate: immediate SmartScreen reputation, requires a hardware token or cloud HSM.

Once a certificate or signing service is chosen, sign vaultzip-gui.exe and vaultzip.exe before the installer is built, then sign the installer itself (Inno Setup supports this with the SignTool directive). Until then, publish the SHA-256 checksums with every release, as the workflow already does.
