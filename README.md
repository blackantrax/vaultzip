# VaultZip

A free, open-source archiver with strong password protection. Create and extract ZIP archives, and lock them with AES-256 encryption. Built to be simple for everyone and trustworthy for security teams.

VaultZip runs on Windows, macOS and Linux. Windows is the primary target.

## Status

Early development (v0.1). The core library and command line tool work today. A desktop interface with drag and drop is next on the roadmap.

## Features

- Create and extract ZIP archives
- AES-256 encryption (WinZip AES, authenticated). Legacy ZipCrypto is never used for writing
- Zip-slip protection: entries that try to escape the destination folder are rejected
- Password confirmation and length warning when encrypting
- Single static binary, no runtime required

## Quick start

Build from source (Rust 1.75 or newer):

    cargo build --release

Create an encrypted archive (you will be prompted for a password):

    vaultzip create backup.zip Documents Photos --encrypt

Extract an archive:

    vaultzip extract backup.zip --dest restored

List contents:

    vaultzip list backup.zip

## Project layout

- crates/vaultzip-core: archive engine (library)
- crates/vaultzip-cli: command line interface
- docs/ROADMAP.md: planned work

## Security

Encrypted archives protect file contents. File names remain visible in the ZIP format. A lost password cannot be recovered. See SECURITY.md for how to report vulnerabilities.

## License

MIT. See LICENSE.
