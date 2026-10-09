# First release checklist

Work through these in order. The first three sections happen before the repository becomes public, because they are hard to undo afterwards.

## 1. Decisions to make before going public

- [ ] Name: search GitHub, crates.io and trademark databases (CIPO in Canada, USPTO in the United States) for VaultZip and similar names. If it is taken or risky, rename now. The name appears in Cargo.toml, the crates, installer/vaultzip.iss, the README and the workflows.
- [ ] Copyright holder: LICENSE currently says Omerta Security. Confirm that is the legal owner you want, since a buyer will check this. Use the full legal entity name if you prefer.
- [ ] Contributions and ownership: if you plan to sell or relicense the project, outside contributions complicate ownership. Decide between requiring a Contributor License Agreement, requiring a Developer Certificate of Origin sign-off on every commit, or accepting contributions only after discussion. Record the choice in CONTRIBUTING.md.
- [ ] Licensing review: confirm every dependency is permissively licensed. Run cargo-deny or cargo-about and keep the output, because a buyer will ask for a third-party license inventory. Have a lawyer review the licensing before any sale.
- [ ] Security contact: SECURITY.md refers to GitHub private vulnerability reporting. Decide who receives those reports and make sure they will see them.

## 2. Create the repository

- [ ] Create the repository on your GitHub organization or account. Do not initialize it with a README or license.
- [ ] Replace the placeholder owner: search for OWNER. It appears in Cargo.toml (repository) and .github/ISSUE_TEMPLATE/config.yml (security report link).
- [ ] Push the code.
- [ ] Settings, Code security: enable private vulnerability reporting, Dependabot alerts, Dependabot security updates and secret scanning.
- [ ] Settings, Branches: protect main. Require pull requests, require the CI checks to pass (all three test jobs and the installer job), and block force pushes.
- [ ] Settings, Actions, General: set workflow permissions to read only by default. The release workflow requests what it needs.
- [ ] Turn on two-factor authentication for every account with write access.
- [ ] Settings, Environments: create an environment named release and add yourself as a required reviewer.

## 3. Get CI green

- [ ] The first run of the CI workflow passes on Windows, macOS and Linux.
- [ ] The installer job passes. It builds the installer, installs it silently, checks the files and registry entries, uninstalls it and checks the cleanup. If it fails, read the failing step; it names exactly what did not match.
- [ ] Download the installer artifact from the run for the manual tests below.

## 4. Test on a real Windows machine

Use the installer artifact from CI, on a Windows 10 or 11 machine.

- [ ] Install with the default options. No administrator prompt appears.
- [ ] The Start menu shows VaultZip with the padlock icon. Properties on the executable show the version and publisher.
- [ ] Right-click a file, a folder and a ZIP. The VaultZip entries appear (on Windows 11, under Show more options).
- [ ] Select five files and choose Add to password-protected archive. A single window opens with all five listed.
- [ ] Create the archive with a strong password. The strength meter reacts as you type.
- [ ] Open the archive in 7-Zip. It asks for the password and extracts correctly.
- [ ] Select two ZIP files and choose Extract here. Both extract into the right folders.
- [ ] Extract a password-protected archive with a wrong password. A clear error appears and no files are left behind.
- [ ] Archive a large folder (a few gigabytes), watch the progress bar, then cancel. No partial archive remains and the window is usable.
- [ ] Open the app Settings, remove the menu, then add it again.
- [ ] Uninstall from Settings, Apps. The program, the shortcut and every right-click entry disappear.
- [ ] Try the same steps with a path that contains spaces, accented characters and a very long folder name.

## 5. Test on macOS and Linux

- [ ] The desktop app starts and creates and extracts a password-protected archive.
- [ ] The command line tool works: create with --encrypt, list, extract.
- [ ] The archive opens in 7-Zip or p7zip on the same system.

## 6. Release candidate

- [ ] Push a tag such as v0.1.0-rc1. Releases with a hyphen in the tag are marked as pre-releases automatically.
- [ ] Approve the release in the release environment when GitHub asks.
- [ ] The release page lists the Windows installer, the portable Windows files, the macOS and Linux files and a SHA256SUMS file for each platform.
- [ ] Download the Windows installer and verify its checksum with certutil -hashfile (file name) SHA256 and compare it to the published value.
- [ ] If signing is enabled, check the Digital Signatures tab on the installer and confirm the publisher name is correct. If not enabled, expect the SmartScreen warning and mention it in the release notes.
- [ ] Install the release candidate on a clean machine and repeat a short version of section 4.

## 7. Publish v0.1.0

- [ ] Fix anything found with the release candidate, update the version in the root Cargo.toml and tag v0.1.0.
- [ ] Edit the generated release notes. Lead with what the app does, how to install it, and the known limits: file names inside ZIP archives are visible, a lost password cannot be recovered, and Windows Explorer cannot open AES-encrypted ZIP files.
- [ ] Read the README as a first-time visitor. Add a screenshot of the app if you can.
- [ ] Enable GitHub Discussions if you want questions kept out of the issue tracker.

## 8. After release

- [ ] Watch the issue tracker for the first week and respond quickly. Early responsiveness decides whether the project looks alive.
- [ ] Merge the Dependabot pull requests when CI passes.
- [ ] Review SECURITY.md after the first report, and fix the process if anything was unclear.
- [ ] Decide on the next milestone from the roadmap and publish it as a GitHub milestone so users can follow progress.
