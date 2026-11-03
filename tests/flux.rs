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

// Saved recordings must be usable without Cargo or any target checkout.
#[test]
fn parse_flux_replays_recordings_without_changing_evidence() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/flux");
    for case in [
        "success",
        "diagnostic",
        "crashes",
        "real_crashes",
        "interrupted",
        "launch_failure",
    ] {
        let temp = Temp::new();
        for name in ["stdout.log", "stderr.log", "result.json"] {
            fs::copy(fixtures.join(case).join(name), temp.0.join(name)).unwrap();
        }
        // An existing report is evidence too: parsing must not replace it.
        fs::write(temp.0.join("errors.json"), "existing report").unwrap();
        let run = Command::new(env!("CARGO_BIN_EXE_fourgate"))
            .args(["parse-flux", "--input"])
            .arg(&temp.0)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(run.status.success(), "{case}: {:?}", run.stderr);
        let actual: Value = serde_json::from_slice(&run.stdout).unwrap();
        let expected: Value =
            serde_json::from_slice(&fs::read(fixtures.join(case).join("expected.json")).unwrap())
                .unwrap();
        assert_eq!(actual, expected, "{case}");
        for name in ["stdout.log", "stderr.log", "result.json"] {
            assert_eq!(
                fs::read(temp.0.join(name)).unwrap(),
                fs::read(fixtures.join(case).join(name)).unwrap()
            );
        }
        assert_eq!(
            fs::read_to_string(temp.0.join("errors.json")).unwrap(),
            "existing report"
        );
    }
}

#[test]
fn parse_flux_handles_missing_results_and_rejects_damaged_logs() {
    let temp = Temp::new();
    fs::write(temp.0.join("stdout.log"), "").unwrap();
    fs::write(temp.0.join("stderr.log"), "").unwrap();
    let parse = || {
        Command::new(env!("CARGO_BIN_EXE_fourgate"))
            .args(["parse-flux", "--input"])
            .arg(&temp.0)
            .output()
            .unwrap()
    };
    let run = parse();
    assert!(run.status.success());
    let report: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(report["complete"], false);
    assert_eq!(report["incomplete_reasons"], json!(["run_interrupted"]));
    fs::write(temp.0.join("stdout.log"), "{").unwrap();
    let run = parse();
    assert!(!run.status.success());
    assert!(run.stdout.is_empty(), "must not publish a partial report");
    assert_eq!(fs::read(temp.0.join("stdout.log")).unwrap(), b"{");
}

#[test]
fn interruption_gate_accepts_diagnostics_but_rejects_catastrophic_failures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/flux");
    for (case, passes) in [
        ("success", true),
        ("diagnostic", true),
        ("crashes", false),
        ("real_crashes", false),
        ("interrupted", false),
        ("launch_failure", false),
        ("malformed_json", false),
    ] {
        let run = Command::new(env!("CARGO_BIN_EXE_fourgate"))
            .args(["gate-flux", "--input"])
            .arg(fixtures.join(case))
            .output()
            .unwrap();
        assert_eq!(run.status.success(), passes, "{case}: {:?}", run.stderr);
    }
}

#[test]
fn interruption_gate_uses_raw_logs_and_requires_finished_readable_evidence() {
    let temp = Temp::new();
    let gate = || {
        Command::new(env!("CARGO_BIN_EXE_fourgate"))
            .args(["gate-flux", "--input"])
            .arg(&temp.0)
            .output()
            .unwrap()
    };
    // Missing logs must fail, even if a report claims success.
    fs::write(temp.0.join("errors.json"), r#"{"complete":true}"#).unwrap();
    assert!(!gate().status.success());
    fs::write(temp.0.join("stdout.log"), "").unwrap();
    fs::write(temp.0.join("stderr.log"), "").unwrap();
    assert!(!gate().status.success(), "missing result must fail");
    fs::write(
        temp.0.join("result.json"),
        r#"{"exit_code":0,"launch_error":null}"#,
    )
    .unwrap();
    assert!(gate().status.success());
    fs::write(
        temp.0.join("stderr.log"),
        "thread 'rustc' panicked at compiler.rs:1:1:\ncrash\n",
    )
    .unwrap();
    let run = gate();
    assert!(
        !run.status.success(),
        "crash must override a zero exit and stale report"
    );
    assert!(String::from_utf8_lossy(&run.stderr).contains("compiler_crash"));
    fs::write(temp.0.join("stdout.log"), "error: ordinary proof failure\n").unwrap();
    fs::write(temp.0.join("stderr.log"), "Caused by:\n  process didn't exit successfully: `flux-driver` (signal: 9, SIGKILL: kill)\n").unwrap();
    let run = gate();
    assert!(
        !run.status.success(),
        "a killed compiler must fail despite other diagnostics"
    );
    assert!(String::from_utf8_lossy(&run.stderr).contains("run_interrupted"));
    fs::write(temp.0.join("stderr.log"), "").unwrap();
    fs::write(temp.0.join("result.json"), "{").unwrap();
    assert!(!gate().status.success(), "malformed result must fail");
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
