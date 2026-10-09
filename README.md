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
- Desktop app with drag and drop, file pickers, password strength meter and background processing
- Single binary, no runtime or installer dependencies

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

## Project layout

- crates/vaultzip-core: archive engine (library)
- crates/vaultzip-cli: command line interface
- crates/vaultzip-gui: desktop application (egui)
- docs/ROADMAP.md: planned work

## Security

Encrypted archives protect file contents. File names remain visible in the ZIP format. A lost password cannot be recovered. See SECURITY.md for how to report vulnerabilities.

## License

MIT. See LICENSE.
