# Testing VaultZip on a Windows PC

This guide takes about 30 minutes. You do not need Rust or any developer tools for parts 1 to 6. GitHub builds the installer for you.

## 1. Get the installer

1. Open the repository on GitHub and click the **Actions** tab.
2. Click the latest **CI** run on `main` with a green check mark.
3. Scroll to **Artifacts** at the bottom and download **VaultZip-installer-ci**. It is a ZIP file that contains `VaultZip-Setup-0.0.0-ci.exe`.
4. Extract it (right-click, Extract All).

The build is not code-signed yet, so Windows SmartScreen will warn "Windows protected your PC". Click **More info**, then **Run anyway**. This is expected until signing is set up (see docs/SIGNING.md).

### Recommended: test inside Windows Sandbox

Windows Sandbox is a throwaway copy of Windows. Everything inside it disappears when you close it, so it is the safest place to try an unsigned build. It is included with Windows 10/11 Pro, Enterprise and Education.

1. Press Start, type **Turn Windows features on or off**, tick **Windows Sandbox**, click OK and restart. Your IT team may need to do this on a managed PC.
2. Start **Windows Sandbox** from the Start menu.
3. Copy the installer on your PC (Ctrl+C) and paste it on the Sandbox desktop (Ctrl+V).

If Sandbox is not available, testing on your normal PC is fine. The installer is per-user, needs no administrator rights and uninstalls cleanly.

## 2. Install

- [ ] Run the installer. **No administrator (UAC) prompt** should appear.
- [ ] Keep both integration options ticked (right-click menu and Open with).
- [ ] VaultZip appears in the Start menu with the padlock icon.
- [ ] Right-click `vaultzip-gui.exe` in `%LOCALAPPDATA%\Programs\VaultZip`, then Properties, Details. The product name and version are shown.

## 3. Create archives

Make a test folder, for example `C:\VZTest`, with a few files of different kinds (a text file, a photo, a PDF) and a subfolder.

- [ ] **Plain archive:** open VaultZip, untick password protection, drop the folder in, click Create archive. The ZIP opens in Windows Explorer and the content is correct.
- [ ] **Encrypted archive:** tick password protection. Type a weak password such as `abc123`. The strength meter shows red. Type a long one, for example `Correct-Horse-Battery-42`. It turns green.
- [ ] Type a different value in Confirm. "Passwords do not match" appears and Create archive is disabled.
- [ ] Create the encrypted archive. The archive opens in Windows Explorer, but the files inside cannot be read. This is expected: Explorer does not support AES encryption.
- [ ] **Compression levels:** archive the same folder of text files or e-mails four times, once with each Compression setting (Fast, Normal, Maximum, Ultra), saving under a different name each time. Each level is the same size or smaller than the one before. Ultra is noticeably slower. All four open in 7-Zip and in Windows Explorer (when not encrypted).
- [ ] **Already-compressed files:** archive a folder of photos or videos at Ultra. It finishes about as fast as Fast, because those files are stored without recompressing.
- [ ] **Progress and cancel:** archive a large folder (a few GB, for example a Videos folder) and click Cancel halfway. No `.zip` or `.zip.part` file is left behind.

## 4. Extract archives

- [ ] Open the encrypted archive in VaultZip, enter the **wrong** password and click Extract. A clear "incorrect password" error appears and **no file is left** in the destination folder.
- [ ] Extract again with the right password. The files are identical to the originals.
- [ ] Extract into the same folder a second time. VaultZip stops with "file already exists, not overwritten" and **your existing files are unchanged**.

## 5. Explorer right-click menu

On Windows 11, the entries are under **Show more options** (or Shift+right-click).

- [ ] Select 5 files, right-click, then **VaultZip > Add to password-protected archive**. **One** window opens with all 5 files listed.
- [ ] Right-click a folder, then **Add to archive**. The folder is added.
- [ ] Right-click a ZIP, then **Extract to folder**. A folder named after the ZIP is created.
- [ ] Select two ZIP files, right-click, then **Extract here**. Both are extracted.
- [ ] In the app, open **Settings**, click **Remove menu** (the entries disappear), then **Add menu** (they come back).

## 6. Compatibility with other archivers

Install [7-Zip](https://www.7-zip.org/) (free) for this step.

- [ ] Open a VaultZip encrypted archive with 7-Zip. 7-Zip asks for the password and extracts correctly. Properties show the method as `AES-256 Deflate`.
- [ ] Create an encrypted ZIP with 7-Zip (Archive format: zip, Encryption method: **AES-256**) and open it in VaultZip. It extracts correctly.
- [ ] Try a path with spaces, accents and a long name, such as `C:\VZTest\Dossier été 2026\un nom de fichier très long ... .txt`.

## 7. Security tests

These confirm that VaultZip resists malicious archives. Run them in Windows Sandbox if you can.

### 7a. Path traversal (zip-slip)

A malicious ZIP can try to write files outside the destination folder, for example into your Startup folder. Open **PowerShell** and paste the following. It builds such an archive with standard Windows components and writes nothing outside `C:\VZTest`:

```powershell
Add-Type -AssemblyName System.IO.Compression
New-Item -ItemType Directory -Force C:\VZTest | Out-Null
$zipPath = "C:\VZTest\evil.zip"
Remove-Item $zipPath -ErrorAction SilentlyContinue
$fs = [IO.File]::Open($zipPath, 'Create')
$zip = New-Object IO.Compression.ZipArchive($fs, 'Create')
foreach ($name in @('../../escaped.txt', 'safe.txt')) {
    $w = New-Object IO.StreamWriter($zip.CreateEntry($name).Open())
    $w.Write('test'); $w.Close()
}
$zip.Dispose(); $fs.Close()
"Created $zipPath"
```

- [ ] Right-click `evil.zip`, then **VaultZip > Extract to folder**. VaultZip reports **"unsafe path in archive: ../../escaped.txt"**.
- [ ] Check that `C:\escaped.txt` does **not** exist.

### 7b. Hidden data streams

Same as above, but replace the entry list with `@('notes.txt:hidden')`.

- [ ] VaultZip refuses with "unsafe path in archive".

### 7c. Tampered encrypted archive

1. Copy your encrypted archive to `tampered.zip`.
2. Open `tampered.zip` in Notepad++ or any hex editor, change a few characters in the **middle** of the file and save. Do not use plain Notepad, which can damage the whole file.
3. Extract it with VaultZip and the right password.

- [ ] Extraction fails with an error and the damaged file is **not** left on disk. AES authentication caught the change.

### 7d. Passwords never touch the disk

- [ ] After creating and extracting encrypted archives, search `%LOCALAPPDATA%\Programs\VaultZip` and `%TEMP%` for your test password (in Explorer, search `content:Correct-Horse`). Nothing is found.

## 8. Command line tool

In PowerShell:

```powershell
$vz = "$env:LOCALAPPDATA\Programs\VaultZip\vaultzip.exe"
& $vz --version
& $vz create C:\VZTest\cli.zip C:\VZTest\safe.txt --encrypt --level ultra
& $vz list C:\VZTest\cli.zip
& $vz extract C:\VZTest\cli.zip --dest C:\VZTest\cli-out
```

- [ ] The password is not shown while you type it.
- [ ] `list` marks each file `enc`.
- [ ] Extraction asks for the password and restores the file.

## 9. Uninstall

- [ ] Open Settings, Apps, Installed apps, then VaultZip, then Uninstall.
- [ ] The Start menu entry, the program folder and every right-click entry are gone. Restart Explorer or sign out and back in if a menu entry seems to linger.

## Reporting what you find

For each failed check, note the step number, what you expected, what happened and the exact error text. Open a GitHub issue with the **Bug report** template. Report security problems privately through **Security > Report a vulnerability**, never in a public issue. Never attach archives that contain private data, and never share a real password.

## Optional: build from source

Only needed if you want to change the code.

1. Install Rust from https://rustup.rs. Accept the prompt to install the Visual Studio C++ Build Tools.
2. Install Inno Setup 6 from https://jrsoftware.org/isinfo.php.
3. In PowerShell, in the repository folder:

```powershell
cargo test --workspace
cargo build --release
.\installer\build-installer.ps1 -Version 0.1.0 -BuildDir target\release
.\installer\smoke-test.ps1 -Installer dist\VaultZip-Setup-0.1.0.exe
```
