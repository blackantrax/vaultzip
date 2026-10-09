#![cfg_attr(not(windows), allow(dead_code))]

//! Windows Explorer right-click integration.
//!
//! Menu entries are written under HKEY_CURRENT_USER, so no administrator
//! rights are needed and nothing outside the user's profile is touched.
//! On Windows 11 the entries appear under "Show more options".

use std::path::Path;

pub struct Verb {
    pub key: &'static str,
    pub label: &'static str,
    pub args: &'static str,
}

pub struct Menu {
    pub class_path: &'static str,
    pub verbs: &'static [Verb],
}

const ADD_VERBS: &[Verb] = &[
    Verb {
        key: "01add",
        label: "Add to archive...",
        args: "--add",
    },
    Verb {
        key: "02protect",
        label: "Add to password-protected archive...",
        args: "--add --encrypt",
    },
];

const ZIP_VERBS: &[Verb] = &[
    Verb {
        key: "01open",
        label: "Open with VaultZip",
        args: "--open",
    },
    Verb {
        key: "02here",
        label: "Extract here",
        args: "--extract-here",
    },
    Verb {
        key: "03folder",
        label: "Extract to folder",
        args: "--extract-folder",
    },
];

pub const MENUS: &[Menu] = &[
    Menu {
        class_path: r"Software\Classes\*\shell\VaultZip",
        verbs: ADD_VERBS,
    },
    Menu {
        class_path: r"Software\Classes\Directory\shell\VaultZip",
        verbs: ADD_VERBS,
    },
    Menu {
        class_path: r"Software\Classes\SystemFileAssociations\.zip\shell\VaultZip",
        verbs: ZIP_VERBS,
    },
];

/// The command Explorer runs for one verb.
pub fn command_line(exe: &Path, args: &str) -> String {
    format!("\"{}\" {} \"%1\"", exe.display(), args)
}

pub fn supported() -> bool {
    cfg!(windows)
}

#[cfg(windows)]
mod imp {
    use super::{command_line, MENUS};
    use std::io::ErrorKind;
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    fn err(e: std::io::Error) -> String {
        format!("Registry error: {e}")
    }

    pub fn install() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_text = exe.display().to_string();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for menu in MENUS {
            let (key, _) = hkcu.create_subkey(menu.class_path).map_err(err)?;
            key.set_value("MUIVerb", &"VaultZip".to_string())
                .map_err(err)?;
            key.set_value("SubCommands", &String::new()).map_err(err)?;
            key.set_value("Icon", &exe_text).map_err(err)?;
            for verb in menu.verbs {
                let (vk, _) = key
                    .create_subkey(format!(r"shell\{}", verb.key))
                    .map_err(err)?;
                vk.set_value("MUIVerb", &verb.label.to_string())
                    .map_err(err)?;
                let (ck, _) = vk.create_subkey("command").map_err(err)?;
                ck.set_value("", &command_line(&exe, verb.args))
                    .map_err(err)?;
            }
        }
        Ok(())
    }

    pub fn uninstall() -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for menu in MENUS {
            match hkcu.delete_subkey_all(menu.class_path) {
                Ok(()) => {}
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(err(e)),
            }
        }
        Ok(())
    }

    pub fn is_installed() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(MENUS[0].class_path)
            .is_ok()
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn install() -> Result<(), String> {
        Err("Explorer integration is only available on Windows.".to_string())
    }
    pub fn uninstall() -> Result<(), String> {
        Err("Explorer integration is only available on Windows.".to_string())
    }
    pub fn is_installed() -> bool {
        false
    }
}

pub use imp::{install, is_installed, uninstall};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_lines_quote_the_executable_and_the_item() {
        let c = command_line(
            Path::new(r"C:\Program Files\VaultZip\vaultzip-gui.exe"),
            "--add --encrypt",
        );
        assert_eq!(
            c,
            r#""C:\Program Files\VaultZip\vaultzip-gui.exe" --add --encrypt "%1""#
        );
    }

    #[test]
    fn every_verb_maps_to_a_known_option() {
        for menu in MENUS {
            assert!(menu.class_path.starts_with(r"Software\Classes\"));
            for v in menu.verbs {
                let args: Vec<String> = v
                    .args
                    .split(' ')
                    .map(|s| s.to_string())
                    .chain(std::iter::once("x".to_string()))
                    .collect();
                assert!(crate::launch::parse(args).is_ok(), "bad verb {}", v.key);
            }
        }
    }

    #[test]
    fn verb_keys_sort_in_menu_order() {
        for menu in MENUS {
            let keys: Vec<&str> = menu.verbs.iter().map(|v| v.key).collect();
            let mut sorted = keys.clone();
            sorted.sort();
            assert_eq!(keys, sorted);
        }
    }
}
