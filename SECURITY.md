# Security Policy

## Reporting a vulnerability

Please do not open a public issue for security problems. Use GitHub private vulnerability reporting on this repository, or email the maintainers at the address listed on the organization profile. We aim to acknowledge reports within 3 business days and to publish a fix and advisory within 90 days.

## Scope

In scope: archive parsing and extraction, encryption and decryption, path handling, password handling, and the release pipeline.

## Design commitments

- AES-256 only for encryption. No legacy ZipCrypto for new archives
- Extraction never writes outside the chosen destination
- Encrypted entries are authenticated while they are extracted; a file that fails authentication is deleted and extraction stops
- Extraction never overwrites an existing file
- Passwords are never logged or written to disk

## Known limitations

- File names inside ZIP archives are not encrypted
- Archives are not padded, so approximate sizes may be inferred
