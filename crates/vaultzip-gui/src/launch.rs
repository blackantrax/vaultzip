//! Command line handling for launches from Windows Explorer, plus a small
//! file-based queue that merges several simultaneous launches (one per
//! selected file) into a single window.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Add,
    Open,
    ExtractHere,
    ExtractFolder,
    InstallShell,
    UninstallShell,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Launch {
    pub mode: Mode,
    pub encrypt: bool,
    /// Suppress dialogs (used by the installer).
    pub silent: bool,
    pub paths: Vec<PathBuf>,
}

fn is_zip(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Launch, String> {
    let mut launch = Launch::default();
    let mut explicit = false;
    let mut set_mode = |launch: &mut Launch, mode: Mode| -> Result<(), String> {
        if explicit && launch.mode != mode {
            return Err("conflicting options".to_string());
        }
        explicit = true;
        launch.mode = mode;
        Ok(())
    };
    for arg in args {
        match arg.as_str() {
            "--add" => set_mode(&mut launch, Mode::Add)?,
            "--open" => set_mode(&mut launch, Mode::Open)?,
            "--extract-here" => set_mode(&mut launch, Mode::ExtractHere)?,
            "--extract-folder" => set_mode(&mut launch, Mode::ExtractFolder)?,
            "--install-shell" => set_mode(&mut launch, Mode::InstallShell)?,
            "--uninstall-shell" => set_mode(&mut launch, Mode::UninstallShell)?,
            "--encrypt" => launch.encrypt = true,
            "--silent" => launch.silent = true,
            s if s.starts_with("--") => return Err(format!("unknown option: {s}")),
            _ => launch.paths.push(PathBuf::from(arg)),
        }
    }
    if !explicit && !launch.paths.is_empty() {
        // Files dropped on the program icon or opened with VaultZip.
        if launch.paths.len() == 1 && is_zip(&launch.paths[0]) {
            launch.mode = Mode::Open;
        } else {
            launch.mode = Mode::Add;
            launch.encrypt = true;
        }
    }
    let needs_paths = matches!(
        launch.mode,
        Mode::Add | Mode::Open | Mode::ExtractHere | Mode::ExtractFolder
    );
    if needs_paths && launch.paths.is_empty() {
        return Err("no files were given".to_string());
    }
    Ok(launch)
}

/// Name of the queue folder for a launch, so different actions never mix.
pub fn queue_key(launch: &Launch) -> &'static str {
    match (launch.mode, launch.encrypt) {
        (Mode::Add, true) => "add-protected",
        (Mode::Add, false) => "add",
        (Mode::Open, _) => "open",
        (Mode::ExtractHere, _) => "extract-here",
        (Mode::ExtractFolder, _) => "extract-folder",
        _ => "other",
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(0);
const MAX_ENTRY_AGE: Duration = Duration::from_secs(60);
const STALE_LOCK_AGE: Duration = Duration::from_secs(30);

fn try_lock(lock: &Path) -> io::Result<bool> {
    let create = || OpenOptions::new().write(true).create_new(true).open(lock);
    match create() {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            let stale = fs::metadata(lock)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > STALE_LOCK_AGE);
            if stale {
                let _ = fs::remove_file(lock);
                return Ok(create().is_ok());
            }
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    files.sort();
    for f in files {
        let fresh = fs::metadata(&f)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_none_or(|age| age < MAX_ENTRY_AGE);
        if fresh {
            if let Ok(text) = fs::read_to_string(&f) {
                for line in text.lines().filter(|l| !l.is_empty()) {
                    let p = PathBuf::from(line);
                    if !out.contains(&p) {
                        out.push(p);
                    }
                }
            }
        }
        let _ = fs::remove_file(&f);
    }
    Ok(())
}

fn has_entries(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .any(|e| e.path().extension().is_some_and(|x| x == "txt"))
        })
        .unwrap_or(false)
}

/// Register `mine` in the queue. Exactly one concurrent caller becomes the
/// leader and receives every path queued during the settle window; the others
/// get `None` and should exit.
pub fn coalesce(
    dir: &Path,
    mine: &[PathBuf],
    settle: Duration,
) -> io::Result<Option<Vec<PathBuf>>> {
    fs::create_dir_all(dir)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let id = format!(
        "{}-{}-{}",
        std::process::id(),
        stamp,
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let body: String = mine
        .iter()
        .filter_map(|p| p.to_str())
        .filter(|s| !s.contains('\n'))
        .map(|s| format!("{s}\n"))
        .collect();
    // Write then rename, so the leader never reads a half-written entry.
    let tmp = dir.join(format!("{id}.tmp"));
    fs::write(&tmp, body)?;
    fs::rename(&tmp, dir.join(format!("{id}.txt")))?;

    let lock = dir.join("leader.lock");
    if !try_lock(&lock)? {
        return Ok(None);
    }
    let mut all = Vec::new();
    loop {
        std::thread::sleep(settle);
        collect(dir, &mut all)?;
        let _ = fs::remove_file(&lock);
        // Pick up anyone who arrived while the lock was being released.
        if !has_entries(dir) || !try_lock(&lock)? {
            break;
        }
    }
    Ok(Some(all))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_shell_verbs() {
        let l = parse(args(&["--add", "--encrypt", "C:\\a.txt"])).unwrap();
        assert_eq!(l.mode, Mode::Add);
        assert!(l.encrypt);
        assert_eq!(l.paths, vec![PathBuf::from("C:\\a.txt")]);

        let l = parse(args(&["--extract-here", "x.zip"])).unwrap();
        assert_eq!(l.mode, Mode::ExtractHere);
        let l = parse(args(&["--install-shell"])).unwrap();
        assert_eq!(l.mode, Mode::InstallShell);
        assert!(!l.silent);
        let l = parse(args(&["--uninstall-shell", "--silent"])).unwrap();
        assert_eq!(l.mode, Mode::UninstallShell);
        assert!(l.silent);
    }

    #[test]
    fn bare_paths_pick_a_sensible_mode() {
        assert_eq!(parse(args(&["a.ZIP"])).unwrap().mode, Mode::Open);
        let l = parse(args(&["a.txt", "b.txt"])).unwrap();
        assert_eq!(l.mode, Mode::Add);
        assert!(l.encrypt);
        assert_eq!(parse(args(&[])).unwrap().mode, Mode::Normal);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(args(&["--bogus"])).is_err());
        assert!(parse(args(&["--add"])).is_err());
        assert!(parse(args(&["--add", "--open", "x"])).is_err());
    }

    #[test]
    fn simultaneous_launches_merge_into_one_leader() {
        let t = tempdir().unwrap();
        let dir = Arc::new(t.path().join("q"));
        let names: Vec<PathBuf> = (0..6)
            .map(|i| PathBuf::from(format!("C:\\file{i}.txt")))
            .collect();
        let handles: Vec<_> = names
            .iter()
            .cloned()
            .map(|p| {
                let dir = Arc::clone(&dir);
                std::thread::spawn(move || {
                    coalesce(&dir, &[p], Duration::from_millis(400)).unwrap()
                })
            })
            .collect();
        let results: Vec<Option<Vec<PathBuf>>> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();
        let mut merged: Vec<PathBuf> = results.iter().flatten().flatten().cloned().collect();
        merged.sort();
        let mut expected = names.clone();
        expected.sort();
        assert_eq!(merged, expected, "every path is delivered exactly once");
        assert!(results.iter().any(|r| r.is_some()), "someone must lead");
    }

    #[test]
    fn stale_entries_are_ignored_and_lock_is_released() {
        let t = tempdir().unwrap();
        let dir = t.path().join("q");
        let got = coalesce(&dir, &[PathBuf::from("one")], Duration::from_millis(50))
            .unwrap()
            .unwrap();
        assert_eq!(got, vec![PathBuf::from("one")]);
        assert!(!dir.join("leader.lock").exists());
        let again = coalesce(&dir, &[PathBuf::from("two")], Duration::from_millis(50))
            .unwrap()
            .unwrap();
        assert_eq!(again, vec![PathBuf::from("two")]);
    }
}
