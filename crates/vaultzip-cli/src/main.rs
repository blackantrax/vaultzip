use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use vaultzip_core as core;

#[derive(Clone, Copy, ValueEnum)]
enum Level {
    /// Quickest, slightly larger archives
    Fast,
    /// Good balance (default)
    Normal,
    /// Best standard compression
    Maximum,
    /// Smallest archives, much slower; best for text and documents
    Ultra,
}

impl From<Level> for core::Level {
    fn from(l: Level) -> Self {
        match l {
            Level::Fast => core::Level::Fast,
            Level::Normal => core::Level::Normal,
            Level::Maximum => core::Level::Maximum,
            Level::Ultra => core::Level::Ultra,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "vaultzip",
    version,
    about = "Free archiver with AES-256 password protection"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a ZIP archive from files and folders
    Create {
        /// Output archive path (for example backup.zip)
        output: PathBuf,
        /// Files and folders to add
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Encrypt with AES-256 (prompts for a password)
        #[arg(short, long)]
        encrypt: bool,
        /// Compression level
        #[arg(short, long, value_enum, default_value_t = Level::Normal)]
        level: Level,
    },
    /// Extract an archive
    Extract {
        archive: PathBuf,
        /// Destination folder
        #[arg(short, long, default_value = ".")]
        dest: PathBuf,
    },
    /// List archive contents
    List { archive: PathBuf },
}

fn prompt(confirm: bool) -> Result<String, String> {
    let p = rpassword::prompt_password("Password: ").map_err(|e| e.to_string())?;
    if confirm {
        let c = rpassword::prompt_password("Confirm password: ").map_err(|e| e.to_string())?;
        if p != c {
            return Err("passwords do not match".into());
        }
        if p.chars().count() < 12 {
            eprintln!("Warning: use 12 or more characters. A lost password cannot be recovered.");
        }
    }
    Ok(p)
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Create {
            output,
            inputs,
            encrypt,
            level,
        } => {
            let pw = if encrypt { Some(prompt(true)?) } else { None };
            let progress = core::Progress::new();
            let level: core::Level = level.into();
            core::create_archive_with_progress(&inputs, &output, pw.as_deref(), level, &progress)
                .map_err(|e| e.to_string())?;
            println!("Created {}", output.display());
        }
        Command::Extract { archive, dest } => {
            let needs_pw = core::list_archive(&archive)
                .map_err(|e| e.to_string())?
                .iter()
                .any(|e| e.encrypted);
            let pw = if needs_pw { Some(prompt(false)?) } else { None };
            core::extract_archive(&archive, &dest, pw.as_deref()).map_err(|e| e.to_string())?;
            println!("Extracted to {}", dest.display());
        }
        Command::List { archive } => {
            for e in core::list_archive(&archive).map_err(|e| e.to_string())? {
                println!(
                    "{:>12}  {}  {}",
                    e.size,
                    if e.encrypted { "enc" } else { "   " },
                    e.name
                );
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}
