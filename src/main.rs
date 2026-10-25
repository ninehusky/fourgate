use serde_json::json;
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
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

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 5 || args[0] != "record" || args[3] != "--" {
        return Err("usage: fourgate record REPO NEW_TASK_DIR -- COMMAND [ARGS...]".into());
    }
    let repo = fs::canonicalize(&args[1])?;
    let task = Path::new(&args[2]);
    // Never overwrite a previous attempt's evidence.
    fs::create_dir(task)?;
    let task = fs::canonicalize(task)?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let command = &args[4..];
    let flux = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/flux");
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&repo)
        .output()?;
    let invocation = json!({
        "schema_version": 1, "stage": 0, "task_id": task.file_name().unwrap().to_string_lossy(),
        "parent_task_id": null, "timestamp_ms": timestamp,
        "cwd": repo, "argv": command, "target_revision": revision(&repo),
        "target_status": String::from_utf8_lossy(&dirty.stdout),
        "flux_revision": revision(&flux),
        "environment": {"RUSTUP_TOOLCHAIN": env::var("RUSTUP_TOOLCHAIN").ok(), "RUSTFLAGS": env::var("RUSTFLAGS").ok()},
        "note": "Environment is partial; record other required variables in summary.md. Never record secrets."
    });
    fs::write(
        task.join("invocation.json"),
        serde_json::to_vec_pretty(&invocation)?,
    )?;
    let mut events = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(task.join("events.jsonl"))?;
    writeln!(
        events,
        "{}",
        json!({"type":"command_started", "timestamp_ms":timestamp, "task_id":invocation["task_id"], "stage":0})
    )?;
    events.flush()?;
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
    writeln!(
        events,
        "{}",
        json!({"type":"command_finished", "timestamp_ms":SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(), "task_id":invocation["task_id"], "stage":0, "exit_code":code, "launch_error":error})
    )?;
    println!("{}", task.display());
    Ok(code.unwrap_or(1))
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            let _ = writeln!(io::stderr(), "fourgate: {error}");
            std::process::exit(1);
        }
    }
}
