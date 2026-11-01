#![cfg(unix)]

use serde_json::{Value, json};
use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let path = env::temp_dir().join(format!(
            "fourgate-flux-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

// Run the real fourgate CLI against a fake Cargo that replays fixture logs.
// Check automatic reports, command forwarding, preserved evidence, and rerun protection.
#[test]
fn flux_packages_errors_preserves_logs_and_returns_child_exit_code() {
    // 1. Isolate the target and fake executable; spaces exercise path handling.
    let temp = Temp::new();
    let repo = temp.0.join("repo with spaces");
    let bin = temp.0.join("bin");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(&bin).unwrap();
    executable(
        &bin.join("cargo"),
        "#!/bin/sh\nprintf '%s\\n' \"$PWD\" > \"$TEST_CWD\"\nprintf '%s\\n' \"$@\" > \"$TEST_ARGS\"\n/bin/cat \"$TEST_STDOUT\"\n/bin/cat \"$TEST_STDERR\" >&2\nexit \"$TEST_EXIT\"\n",
    );
    for (case, exit) in [("success", 0), ("diagnostic", 101), ("real_crashes", 101)] {
        // 2. Replay each recording through the CLI with its original exit status.
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/flux")
            .join(case);
        let output = temp.0.join(case);
        let run = Command::new(env!("CARGO_BIN_EXE_fourgate"))
            .args(["flux", "--repo"])
            .arg(&repo)
            .arg("--output")
            .arg(&output)
            .args(["--", "--workspace", "--keep-going"])
            .env(
                "PATH",
                env::join_paths(
                    std::iter::once(bin.clone())
                        .chain(env::split_paths(&env::var_os("PATH").unwrap())),
                )
                .unwrap(),
            )
            .env("TEST_CWD", temp.0.join("cwd"))
            .env("TEST_ARGS", temp.0.join("args"))
            .env("TEST_STDOUT", fixture.join("stdout.log"))
            .env("TEST_STDERR", fixture.join("stderr.log"))
            .env("TEST_EXIT", exit.to_string())
            .output()
            .unwrap();
        // Packaging a report must preserve Cargo's success or failure exit code.
        assert_eq!(
            run.status.code(),
            Some(exit),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let actual: Value =
            serde_json::from_slice(&fs::read(output.join("errors.json")).unwrap()).unwrap();
        let expected: Value =
            serde_json::from_slice(&fs::read(fixture.join("expected.json")).unwrap()).unwrap();
        // The CLI must write the same report as direct parsing of this fixture.
        assert_eq!(actual, expected, "{case}");
        for name in ["stdout.log", "stderr.log"] {
            // Normalization must leave both raw streams byte-for-byte intact.
            assert_eq!(
                fs::read(output.join(name)).unwrap(),
                fs::read(fixture.join(name)).unwrap()
            );
        }
        // Forward the subcommand, JSON format, and user arguments in order.
        assert_eq!(
            fs::read_to_string(temp.0.join("args")).unwrap(),
            "flux\n--message-format=json\n--workspace\n--keep-going\n"
        );
        // Cargo must run in the supplied repository, including paths with spaces.
        assert_eq!(
            fs::read_to_string(temp.0.join("cwd")).unwrap().trim(),
            fs::canonicalize(&repo).unwrap().to_str().unwrap()
        );
        let result: Value =
            serde_json::from_slice(&fs::read(output.join("result.json")).unwrap()).unwrap();
        // Recorder metadata must retain the child's exit code independently of the report.
        assert_eq!(result["exit_code"], json!(exit));
        // 3. Attempt to reuse the output directory to check evidence protection.
        let report_before = fs::read(output.join("errors.json")).unwrap();
        let repeated = Command::new(env!("CARGO_BIN_EXE_fourgate"))
            .args(["flux", "--repo"])
            .arg(&repo)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        // An existing recording must be rejected rather than overwritten.
        assert!(!repeated.status.success());
        // Rejection must leave the previously generated report unchanged.
        assert_eq!(fs::read(output.join("errors.json")).unwrap(), report_before);
    }
}

// If Cargo cannot start, fourgate must still package a launch-failure report
// using the error recorded in result.json.
#[test]
fn cargo_launch_failure_still_produces_a_report() {
    let temp = Temp::new();
    // The recorder needs Git; Cargo is deliberately absent from this PATH.
    executable(&temp.0.join("git"), "#!/bin/sh\nexit 0\n");
    let output = temp.0.join("run");
    let run = Command::new(env!("CARGO_BIN_EXE_fourgate"))
        .args(["flux", "--repo"])
        .arg(&temp.0)
        .arg("--output")
        .arg(&output)
        .env("PATH", &temp.0)
        .output()
        .unwrap();
    // A launch failure must make the CLI fail, rather than appear successful.
    assert_eq!(run.status.code(), Some(1));
    let report: Value =
        serde_json::from_slice(&fs::read(output.join("errors.json")).unwrap()).unwrap();
    // A command that never ran cannot yield a complete diagnostic inventory.
    assert_eq!(report["complete"], false);
    // The status must identify the launch failure specifically.
    assert_eq!(report["incomplete_reasons"], json!(["invocation_failure"]));
    // The failure must also be available as an error the agent can inspect.
    assert_eq!(report["errors"][0]["kind"], "invocation_failure");
    // Its evidence is recorder metadata, since Cargo produced no output.
    assert_eq!(report["errors"][0]["evidence"][0]["file"], "result.json");
}

// Malformed Cargo output must panic during extraction while preserving the
// original recording and avoiding a misleading generated report.
#[test]
fn extraction_panic_preserves_recording_without_writing_a_report() {
    let temp = Temp::new();
    executable(&temp.0.join("cargo"), "#!/bin/sh\nprintf '{'\nexit 23\n");
    let output = temp.0.join("run");
    let run = Command::new(env!("CARGO_BIN_EXE_fourgate"))
        .args(["flux", "--repo"])
        .arg(&temp.0)
        .arg("--output")
        .arg(&output)
        .env(
            "PATH",
            env::join_paths(
                std::iter::once(temp.0.clone())
                    .chain(env::split_paths(&env::var_os("PATH").unwrap())),
            )
            .unwrap(),
        )
        .output()
        .unwrap();
    // The parser panic must fail the CLI, overriding the child's exit code of 23.
    assert_eq!(run.status.code(), Some(101));
    // The panic must tell the reviewer which raw log line could not be parsed.
    assert!(String::from_utf8_lossy(&run.stderr).contains("invalid JSON at Stdout:1"));
    // A failed extraction must not publish a partial errors.json as a valid report.
    assert!(!output.join("errors.json").exists());
    // The malformed stdout must survive unchanged for debugging.
    assert_eq!(fs::read(output.join("stdout.log")).unwrap(), b"{");
    // The CLI's parser panic must not contaminate the child's recorded stderr.
    assert_eq!(fs::read(output.join("stderr.log")).unwrap(), b"");
    let result: Value =
        serde_json::from_slice(&fs::read(output.join("result.json")).unwrap()).unwrap();
    // The saved result describes Cargo's exit, even though fourgate later panicked.
    assert_eq!(result["exit_code"], 23);
}
