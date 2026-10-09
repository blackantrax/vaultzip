//! VaultZip core: create, list and extract ZIP archives with optional
//! AES-256 encryption. Legacy ZipCrypto is intentionally not supported
//! for writing, and extraction refuses paths that escape the destination.
//!
//! Long operations report progress and can be cancelled through a shared
//! [`Progress`] handle.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use walkdir::WalkDir;
use zip::{
    result::ZipError, write::FileOptions, AesMode, CompressionMethod, ZipArchive, ZipWriter,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("archive error: {0}")]
    Zip(#[from] ZipError),
    #[error("a password is required for this archive")]
    PasswordRequired,
    #[error("incorrect password")]
    InvalidPassword,
    #[error("unsafe path in archive: {0}")]
    UnsafePath(String),
    #[error("input not found: {0}")]
    InputMissing(PathBuf),
    #[error("password must not be empty")]
    EmptyPassword,
    #[error("cancelled")]
    Cancelled,
    #[error("file already exists, not overwritten: {0}")]
    Exists(PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub compressed_size: u64,
    pub is_dir: bool,
    pub encrypted: bool,
}

/// Shared progress and cancellation state for a running operation.
/// Create one, pass a reference to the operation, and read it from
/// another thread to drive a progress bar or call [`Progress::cancel`].
#[derive(Debug, Default)]
pub struct Progress {
    cancel: AtomicBool,
    done: AtomicU64,
    total: AtomicU64,
    current: Mutex<String>,
}

impl Progress {
    pub fn new() -> Self {
        Self::default()
    }

    /// Request cancellation. The operation stops at the next chunk.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Bytes processed so far.
    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    /// Total bytes expected (0 until the operation has scanned its input).
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// Completed fraction between 0.0 and 1.0.
    pub fn fraction(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            return 0.0;
        }
        (self.done() as f64 / total as f64).min(1.0) as f32
    }

    /// Name of the item currently being processed.
    pub fn current(&self) -> String {
        self.current.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// Clear counters so the same handle can be reused for another run.
    pub fn reset(&self) {
        self.cancel.store(false, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
        self.set_current("");
    }

    fn set_total(&self, n: u64) {
        self.total.store(n, Ordering::Relaxed);
    }

    fn add(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
    }

    fn set_current(&self, name: &str) {
        if let Ok(mut c) = self.current.lock() {
            *c = name.to_string();
        }
    }

    fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

const CHUNK: usize = 64 * 1024;

fn copy_with_progress<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    progress: &Progress,
) -> Result<()> {
    let mut buf = vec![0u8; CHUNK];
    loop {
        progress.check()?;
        let n = reader.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        writer.write_all(&buf[..n])?;
        progress.add(n as u64);
    }
}

/// Create a ZIP archive from files and folders. With a password, every
/// file is encrypted with AES-256 (WinZip AES, authenticated).
pub fn create_archive(inputs: &[PathBuf], output: &Path, password: Option<&str>) -> Result<()> {
    create_archive_with_progress(inputs, output, password, &Progress::new())
}

/// Like [`create_archive`], with progress reporting and cancellation.
/// The archive is written to a temporary file next to `output` and moved
/// into place on success, so a failed or cancelled run leaves nothing behind
/// and never damages an existing archive.
pub fn create_archive_with_progress(
    inputs: &[PathBuf],
    output: &Path,
    password: Option<&str>,
    progress: &Progress,
) -> Result<()> {
    if let Some(p) = password {
        if p.is_empty() {
            return Err(Error::EmptyPassword);
        }
    }
    for input in inputs {
        if !input.exists() {
            return Err(Error::InputMissing(input.clone()));
        }
    }
    let part = part_path(output);
    let result = build_archive(inputs, output, &part, password, progress);
    match result {
        Ok(()) => {
            if let Err(e) = fs::rename(&part, output) {
                let _ = fs::remove_file(&part);
                return Err(e.into());
            }
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(&part);
            Err(e)
        }
    }
}

fn part_path(output: &Path) -> PathBuf {
    let mut name = output
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".part");
    output.with_file_name(name)
}

fn build_archive(
    inputs: &[PathBuf],
    output: &Path,
    part: &Path,
    password: Option<&str>,
    progress: &Progress,
) -> Result<()> {
    let skip: Vec<PathBuf> = [output, part]
        .iter()
        .filter_map(|p| std::path::absolute(p).ok())
        .collect();
    let is_skipped = |p: &Path| {
        std::path::absolute(p)
            .map(|a| skip.contains(&a))
            .unwrap_or(false)
    };

    // First pass: total size, so progress is meaningful.
    let mut total = 0u64;
    for input in inputs {
        for item in WalkDir::new(input).follow_links(false) {
            let item = item.map_err(|e| Error::Io(e.into()))?;
            if item.file_type().is_file() && !is_skipped(item.path()) {
                total += item.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        progress.check()?;
    }
    progress.set_total(total);

    let mut writer = ZipWriter::new(File::create(part)?);
    for input in inputs {
        let base = input.parent().unwrap_or_else(|| Path::new(""));
        for item in WalkDir::new(input).follow_links(false) {
            let item = item.map_err(|e| Error::Io(e.into()))?;
            progress.check()?;
            if is_skipped(item.path()) {
                continue;
            }
            let rel = item.path().strip_prefix(base).unwrap_or(item.path());
            let name = to_zip_name(rel);
            if name.is_empty() {
                continue;
            }
            let mut opts =
                FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);
            if let Some(p) = password {
                opts = opts.with_aes_encryption(AesMode::Aes256, p);
            }
            if item.file_type().is_dir() {
                writer.add_directory(name, opts)?;
            } else if item.file_type().is_file() {
                progress.set_current(&name);
                writer.start_file(name, opts)?;
                let mut src = File::open(item.path())?;
                copy_with_progress(&mut src, &mut writer, progress)?;
            }
        }
    }
    writer.finish()?;
    Ok(())
}

/// List the contents of an archive without extracting.
pub fn list_archive(archive: &Path) -> Result<Vec<Entry>> {
    let mut zip = ZipArchive::new(File::open(archive)?)?;
    let mut out = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i)?;
        out.push(Entry {
            name: f.name().to_string(),
            size: f.size(),
            compressed_size: f.compressed_size(),
            is_dir: f.is_dir(),
            encrypted: f.encrypted(),
        });
    }
    Ok(out)
}

/// Extract an archive into `dest`, rejecting any entry that would
/// land outside it (zip-slip protection).
pub fn extract_archive(archive: &Path, dest: &Path, password: Option<&str>) -> Result<()> {
    extract_archive_with_progress(archive, dest, password, &Progress::new())
}

/// Like [`extract_archive`], with progress reporting and cancellation.
/// Files stream to disk in chunks. If a file fails authentication or the run
/// is cancelled, the partially written file is deleted.
pub fn extract_archive_with_progress(
    archive: &Path,
    dest: &Path,
    password: Option<&str>,
    progress: &Progress,
) -> Result<()> {
    let mut zip = ZipArchive::new(File::open(archive)?)?;
    fs::create_dir_all(dest)?;
    let dest = dest.canonicalize()?;

    let mut total = 0u64;
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i)?;
        if !f.is_dir() {
            total += f.size();
        }
    }
    progress.set_total(total);

    for i in 0..zip.len() {
        progress.check()?;
        let encrypted = zip.by_index_raw(i)?.encrypted();
        let mut file = if encrypted {
            let pw = password.ok_or(Error::PasswordRequired)?;
            match zip.by_index_decrypt(i, pw.as_bytes()) {
                Ok(f) => f,
                Err(ZipError::InvalidPassword) => return Err(Error::InvalidPassword),
                Err(e) => return Err(e.into()),
            }
        } else {
            zip.by_index(i)?
        };
        let raw = file.name().to_string();
        let target = safe_join(&dest, &raw)?;
        if file.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        progress.set_current(&raw);
        // Never replace an existing file: extracting into a busy folder must
        // not silently destroy the user's data.
        let mut out = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                return Err(Error::Exists(target));
            }
            Err(e) => return Err(e.into()),
        };
        // Reading to the end also verifies the AES authentication code.
        let copied = copy_with_progress(&mut file, &mut out, progress);
        drop(out);
        if let Err(e) = copied {
            let _ = fs::remove_file(&target);
            return Err(e);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strength {
    VeryWeak,
    Weak,
    Fair,
    Strong,
    VeryStrong,
}

impl Strength {
    pub fn label(self) -> &'static str {
        match self {
            Strength::VeryWeak => "Very weak",
            Strength::Weak => "Weak",
            Strength::Fair => "Fair",
            Strength::Strong => "Strong",
            Strength::VeryStrong => "Very strong",
        }
    }
}

/// Rough password strength estimate based on length, character variety and
/// repetition. It is a guide for users, not a guarantee.
pub fn password_strength(pw: &str) -> Strength {
    const COMMON: [&str; 8] = [
        "password", "123456", "qwerty", "letmein", "welcome", "admin", "iloveyou", "azerty",
    ];
    let lower = pw.to_lowercase();
    if COMMON.iter().any(|c| lower.contains(c)) && pw.chars().count() < 16 {
        return Strength::VeryWeak;
    }
    let len = pw.chars().count();
    if len < 8 {
        return Strength::VeryWeak;
    }
    let mut pool = 0u32;
    if pw.chars().any(|c| c.is_ascii_lowercase()) {
        pool += 26;
    }
    if pw.chars().any(|c| c.is_ascii_uppercase()) {
        pool += 26;
    }
    if pw.chars().any(|c| c.is_ascii_digit()) {
        pool += 10;
    }
    if pw.chars().any(|c| !c.is_ascii_alphanumeric()) {
        pool += 33;
    }
    let mut seen: Vec<char> = pw.chars().collect();
    seen.sort_unstable();
    seen.dedup();
    let effective = seen.len() as f64 + (len - seen.len()) as f64 / 4.0;
    let bits = effective * (pool.max(1) as f64).log2();
    match bits {
        b if b < 40.0 => Strength::Weak,
        b if b < 60.0 => Strength::Fair,
        b if b < 80.0 => Strength::Strong,
        _ => Strength::VeryStrong,
    }
}

fn to_zip_name(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn safe_join(dest: &Path, name: &str) -> Result<PathBuf> {
    // A colon would address an NTFS alternate data stream or a drive on Windows.
    if name.contains(':') {
        return Err(Error::UnsafePath(name.to_string()));
    }
    let mut out = dest.to_path_buf();
    for comp in Path::new(&name.replace('\\', "/")).components() {
        match comp {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            _ => return Err(Error::UnsafePath(name.to_string())),
        }
    }
    if !out.starts_with(dest) {
        return Err(Error::UnsafePath(name.to_string()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample(dir: &Path) -> PathBuf {
        let src = dir.join("docs");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("a.txt"), b"hello world").unwrap();
        fs::write(src.join("sub/b.txt"), b"nested file").unwrap();
        src
    }

    #[test]
    fn roundtrip_plain() {
        let t = tempdir().unwrap();
        let src = sample(t.path());
        let z = t.path().join("out.zip");
        create_archive(&[src], &z, None).unwrap();
        let dest = t.path().join("x");
        extract_archive(&z, &dest, None).unwrap();
        assert_eq!(
            fs::read(dest.join("docs/sub/b.txt")).unwrap(),
            b"nested file"
        );
    }

    #[test]
    fn roundtrip_encrypted() {
        let t = tempdir().unwrap();
        let src = sample(t.path());
        let z = t.path().join("out.zip");
        create_archive(&[src], &z, Some("correct horse")).unwrap();
        assert!(list_archive(&z)
            .unwrap()
            .iter()
            .any(|e| !e.is_dir && e.encrypted));
        let dest = t.path().join("x");
        extract_archive(&z, &dest, Some("correct horse")).unwrap();
        assert_eq!(fs::read(dest.join("docs/a.txt")).unwrap(), b"hello world");
    }

    #[test]
    fn wrong_or_missing_password_fails() {
        let t = tempdir().unwrap();
        let src = sample(t.path());
        let z = t.path().join("out.zip");
        create_archive(&[src], &z, Some("secret")).unwrap();
        let d = t.path().join("x");
        assert!(matches!(
            extract_archive(&z, &d, None),
            Err(Error::PasswordRequired)
        ));
        assert!(extract_archive(&z, &d, Some("nope")).is_err());
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(safe_join(Path::new("/tmp/d"), "../evil.txt").is_err());
        assert!(safe_join(Path::new("/tmp/d"), "/etc/passwd").is_err());
        assert!(safe_join(Path::new("/tmp/d"), "ok/file.txt").is_ok());
        assert!(safe_join(Path::new("/tmp/d"), "a.txt:hidden").is_err());
        assert!(safe_join(Path::new("/tmp/d"), "C:evil.txt").is_err());
    }

    #[test]
    fn extract_never_overwrites_existing_files() {
        let t = tempdir().unwrap();
        let src = sample(t.path());
        let z = t.path().join("out.zip");
        create_archive(&[src], &z, None).unwrap();
        let dest = t.path().join("x");
        fs::create_dir_all(dest.join("docs")).unwrap();
        fs::write(dest.join("docs/a.txt"), b"mine").unwrap();
        let r = extract_archive(&z, &dest, None);
        assert!(matches!(r, Err(Error::Exists(_))));
        assert_eq!(fs::read(dest.join("docs/a.txt")).unwrap(), b"mine");
    }

    #[test]
    fn empty_password_rejected() {
        let t = tempdir().unwrap();
        let src = sample(t.path());
        assert!(matches!(
            create_archive(&[src], &t.path().join("o.zip"), Some("")),
            Err(Error::EmptyPassword)
        ));
    }
}

#[cfg(test)]
mod format_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn encrypted_archive_uses_winzip_aes_not_zipcrypto() {
        let t = tempdir().unwrap();
        let f = t.path().join("a.txt");
        fs::write(&f, b"data").unwrap();
        let z = t.path().join("o.zip");
        create_archive(&[f], &z, Some("pw-123456789")).unwrap();
        let bytes = fs::read(&z).unwrap();
        // Local header: signature, version(2), flags(2), method(2)
        assert_eq!(&bytes[0..4], b"PK\x03\x04");
        let method = u16::from_le_bytes([bytes[8], bytes[9]]);
        assert_eq!(method, 99, "expected WinZip AES marker (method 99)");
    }
}

#[cfg(test)]
mod strength_tests {
    use super::*;

    #[test]
    fn strength_levels() {
        assert_eq!(password_strength("abc"), Strength::VeryWeak);
        assert_eq!(password_strength("Password1"), Strength::VeryWeak);
        assert_eq!(password_strength("aaaaaaaaaaaa"), Strength::Weak);
        assert!(password_strength("correct-Horse-battery-9-staple") >= Strength::Strong);
        assert!(password_strength("kT9#vQ2$mZ7!pL4@") >= Strength::Strong);
    }
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use tempfile::tempdir;

    fn big_file(dir: &Path) -> PathBuf {
        let f = dir.join("big.bin");
        // Compressible but larger than several chunks.
        fs::write(&f, vec![7u8; CHUNK * 5 + 123]).unwrap();
        f
    }

    #[test]
    fn progress_reaches_total_on_create_and_extract() {
        let t = tempdir().unwrap();
        let f = big_file(t.path());
        let z = t.path().join("o.zip");
        let p = Progress::new();
        create_archive_with_progress(&[f], &z, Some("pw-123456789"), &p).unwrap();
        assert_eq!(p.total(), (CHUNK * 5 + 123) as u64);
        assert_eq!(p.done(), p.total());
        assert!((p.fraction() - 1.0).abs() < f32::EPSILON);

        let p2 = Progress::new();
        let dest = t.path().join("x");
        extract_archive_with_progress(&z, &dest, Some("pw-123456789"), &p2).unwrap();
        assert_eq!(p2.done(), p2.total());
        assert_eq!(
            fs::read(dest.join("big.bin")).unwrap().len(),
            CHUNK * 5 + 123
        );
    }

    #[test]
    fn cancelled_create_leaves_no_files() {
        let t = tempdir().unwrap();
        let f = big_file(t.path());
        let z = t.path().join("o.zip");
        let p = Progress::new();
        p.cancel();
        let r = create_archive_with_progress(&[f], &z, None, &p);
        assert!(matches!(r, Err(Error::Cancelled)));
        assert!(!z.exists());
        assert!(!part_path(&z).exists());
    }

    #[test]
    fn failed_create_does_not_damage_existing_archive() {
        let t = tempdir().unwrap();
        let z = t.path().join("o.zip");
        fs::write(&z, b"existing").unwrap();
        let p = Progress::new();
        p.cancel();
        let f = big_file(t.path());
        assert!(create_archive_with_progress(&[f], &z, None, &p).is_err());
        assert_eq!(fs::read(&z).unwrap(), b"existing");
    }

    #[test]
    fn cancelled_extract_removes_partial_output() {
        let t = tempdir().unwrap();
        let f = big_file(t.path());
        let z = t.path().join("o.zip");
        create_archive(&[f], &z, None).unwrap();
        let dest = t.path().join("x");
        let p = Progress::new();
        p.cancel();
        let r = extract_archive_with_progress(&z, &dest, None, &p);
        assert!(matches!(r, Err(Error::Cancelled)));
        assert!(!dest.join("big.bin").exists());
    }

    #[test]
    fn archive_is_not_added_to_itself() {
        let t = tempdir().unwrap();
        let dir = t.path().join("docs");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"hi").unwrap();
        let z = dir.join("docs.zip");
        create_archive(std::slice::from_ref(&dir), &z, None).unwrap();
        let names: Vec<String> = list_archive(&z)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert!(names.iter().any(|n| n == "docs/a.txt"));
        assert!(!names.iter().any(|n| n.contains("docs.zip")));
    }
}
