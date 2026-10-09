//! VaultZip core: create, list and extract ZIP archives with optional
//! AES-256 encryption. Legacy ZipCrypto is intentionally not supported
//! for writing, and extraction refuses paths that escape the destination.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

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

/// Create a ZIP archive from files and folders. With a password, every
/// file is encrypted with AES-256 (WinZip AES, authenticated).
pub fn create_archive(inputs: &[PathBuf], output: &Path, password: Option<&str>) -> Result<()> {
    if let Some(p) = password {
        if p.is_empty() {
            return Err(Error::EmptyPassword);
        }
    }
    let mut writer = ZipWriter::new(File::create(output)?);
    for input in inputs {
        if !input.exists() {
            return Err(Error::InputMissing(input.clone()));
        }
        let base = input.parent().unwrap_or_else(|| Path::new(""));
        for item in WalkDir::new(input).follow_links(false) {
            let item = item.map_err(|e| Error::Io(e.into()))?;
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
                writer.start_file(name, opts)?;
                io::copy(&mut File::open(item.path())?, &mut writer)?;
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
    let mut zip = ZipArchive::new(File::open(archive)?)?;
    fs::create_dir_all(dest)?;
    let dest = dest.canonicalize()?;
    for i in 0..zip.len() {
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
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?; // authenticates AES data before writing
        File::create(&target)?.write_all(&buf)?;
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
