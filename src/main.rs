mod smoke;

use clap::{Args, Parser, Subcommand};
use serde_json::json;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

fn revision(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Capture commands and check the stage 0 fixture. Claude drives the stage itself.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Run a command and save its invocation, raw output, and result.
    Record(Record),
    /// CI-only: check the fixture's specific Flux assertion diagnostic.
    CheckSmoke { directory: PathBuf },
}

#[derive(Args)]
struct Record {
    /// Working directory for the command.
    #[arg(long)]
    repo: PathBuf,
    /// New output directory; its parent must already exist.
    #[arg(long)]
    output: PathBuf,
    /// Executable and arguments, passed directly without shell interpretation.
    #[arg(last = true, required = true, num_args = 1..)]
    command: Vec<String>,
}

fn record(args: Record) -> Result<i32, Box<dyn std::error::Error>> {
    let repo = fs::canonicalize(&args.repo)?;
    let task = &args.output;
    // Never overwrite a previous attempt's evidence.
    fs::create_dir(task)?;
    let task = fs::canonicalize(task)?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let command = &args.command;
    let flux = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/flux");
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&repo)
        .output()?;
    let invocation = json!({
        "schema_version": 1, "timestamp_ms": timestamp,
        "cwd": repo, "argv": command, "target_revision": revision(&repo),
        "target_status": String::from_utf8_lossy(&dirty.stdout),
        "flux_revision": revision(&flux),
        "environment": {"RUSTUP_TOOLCHAIN": env::var("RUSTUP_TOOLCHAIN").ok(), "RUSTFLAGS": env::var("RUSTFLAGS").ok()},
        "note": "Environment is partial; keep other prerequisites in the Claude transcript. Never record secrets."
    });
    fs::write(
        task.join("invocation.json"),
        serde_json::to_vec_pretty(&invocation)?,
    )?;
    let stdout = fs::File::create(task.join("stdout.log"))?;
    let stderr = fs::File::create(task.join("stderr.log"))?;
    let result = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(&repo)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .status();
    let (code, error) = match result {
        Ok(status) => (status.code(), None),
        Err(error) => (None, Some(error.to_string())),
    };
    fs::write(
        task.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "exit_code": code, "launch_error": error,
            "finished_at_ms": SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
        }))?,
    )?;
    println!("{}", task.display());
    Ok(code.unwrap_or(1))
}

fn main() {
    let result = match Cli::parse().command {
        Action::Record(args) => record(args),
        Action::CheckSmoke { directory } => smoke::check(&directory).map(|()| 0),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            let _ = writeln!(io::stderr(), "fourgate: {error}");
            std::process::exit(1);
        }
    }
}
