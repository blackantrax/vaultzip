# VaultZip

A free, open-source archiver with strong password protection. Create and extract ZIP archives, and lock them with AES-256 encryption. Built to be simple for everyone and trustworthy for security teams.

VaultZip runs on Windows, macOS and Linux. Windows is the primary target.

## Status

Early development (v0.1). The core library, command line tool and a first desktop app are in place. Windows is tested first; the desktop app has been compiled and linted on all three platforms but still needs hands-on testing on Windows.

## Features

- Create and extract ZIP archives
- AES-256 encryption (WinZip AES, authenticated). Legacy ZipCrypto is never used for writing
- Zip-slip protection: entries that try to escape the destination folder are rejected
- Password confirmation and length warning when encrypting
- Desktop app with drag and drop, file pickers, password strength meter, progress bar and cancel
- Windows Explorer right-click menu: add to archive, add to password-protected archive, open, extract here, extract to folder
- Safe by default: archives are built in a temporary file and only moved into place when complete, so a cancelled or failed run never leaves a broken archive
- Single binary, no runtime or installer dependencies

## Install on Windows

Download VaultZip-Setup-x.y.z.exe from the Releases page and run it. The installer needs no administrator rights and installs for the current user. It offers to add the Explorer right-click menu and to list VaultZip under Open with for ZIP files. Windows does not let installers take over the default app, so to make VaultZip the default for ZIP files, right-click a ZIP, choose Open with, then Choose another app, select VaultZip and tick Always.

Each release also lists SHA-256 checksums. Verify the download before running it:

    certutil -hashfile VaultZip-Setup-x.y.z.exe SHA256

To uninstall, use Settings, Apps, Installed apps. The right-click menu and Open with entries are removed with it.

## Quick start

Build from source (Rust 1.85 or newer). On Linux, install the GTK 3 development package first (libgtk-3-dev on Debian and Ubuntu):

    cargo build --release

This produces two programs in target/release: vaultzip-gui (desktop app) and vaultzip (command line).

Create an encrypted archive (you will be prompted for a password):

    vaultzip create backup.zip Documents Photos --encrypt

Extract an archive:

    vaultzip extract backup.zip --dest restored

List contents:

    vaultzip list backup.zip

## Compatibility

VaultZip writes standard ZIP files with WinZip AES-256 encryption, the same format used by 7-Zip, WinRAR and WinZip. Password-protected archives open in those programs. The built-in zip support in Windows Explorer and the macOS Archive Utility cannot open AES-encrypted ZIP files, so recipients need VaultZip, 7-Zip or similar. Archives without a password open anywhere.

## Windows Explorer integration

Open the desktop app, expand Settings and choose Add menu. Or run once from a terminal:

    vaultzip-gui.exe --install-shell

To remove it, use Remove menu in Settings or run `vaultzip-gui.exe --uninstall-shell`. The menu is registered for the current user only, so no administrator rights are needed. On Windows 11 the entries appear under Show more options. Run the command again if you move the program to another folder.

Command line options of the desktop app, used by the menu entries: `--add [--encrypt] FILES`, `--open ARCHIVE`, `--extract-here ARCHIVES`, `--extract-folder ARCHIVES`.

## Project layout

- crates/vaultzip-core: archive engine (library)
- crates/vaultzip-cli: command line interface
- crates/vaultzip-gui: desktop application (egui) and Explorer integration
- assets: application icon
- docs/ROADMAP.md: planned work

## Security

Encrypted archives protect file contents. File names remain visible in the ZIP format. A lost password cannot be recovered. See SECURITY.md for how to report vulnerabilities.

## License

MIT. See LICENSE.
