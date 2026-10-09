use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use vaultzip_core as core;

#[derive(Parser)]
#[command(name = "vaultzip", version, about = "Free archiver with AES-256 password protection")]
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
        Command::Create { output, inputs, encrypt } => {
            let pw = if encrypt { Some(prompt(true)?) } else { None };
            core::create_archive(&inputs, &output, pw.as_deref()).map_err(|e| e.to_string())?;
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
                println!("{:>12}  {}  {}", e.size, if e.encrypted { "enc" } else { "   " }, e.name);
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
