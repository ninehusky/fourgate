//! Small source -> expected-JSON pairs, compiled to real ELF executables.
#![cfg(target_os = "linux")]

use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

// Keep the real JSON structure for the two fixture functions, omitting
// build-dependent fields and non-fixture inline frames. Missing or extra sites
// in either before OR after fail, including a panic introduced into after.
fn reviewable(inventory: &Value, source: &Path) -> Value {
    let mut sites = Vec::new();
    for site in inventory["sites"].as_array().unwrap() {
        let name = &site["enclosing_function"]["name"];
        if !matches!(name.as_str(), Some("before" | "after")) {
            continue;
        }
        let frames: Vec<_> = site["attribution"]["frames"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|frame| {
                Path::new(frame["file"].as_str().unwrap_or("")).file_name() == source.file_name()
            })
            .map(|frame| {
                json!({
                    "function": frame["function"],
                    "file": source.file_name().unwrap().to_str().unwrap(),
                    "line": frame["line"],
                })
            })
            .collect();
        sites.push(json!({
            "enclosing_function": {"name": name},
            "kind": site["kind"], "target_symbol": site["target_symbol"],
            "attribution": {"status": site["attribution"]["status"], "frames": frames},
        }));
    }
    json!({"sites": sites})
}

#[test]
fn source_matches_expected_json() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/panics");
    let temp = std::env::temp_dir().join(format!("fourgate-fixtures-{}", std::process::id()));
    fs::create_dir(&temp).unwrap();
    for name in ["bounds", "unwrap", "arithmetic", "slice_copy", "explicit"] {
        let source = fixtures.join(format!("{name}.rs"));
        let binary = temp.join(name);
        let result = Command::new("rustc")
            .arg(&source)
            .args([
                "--edition=2024",
                "-Copt-level=3",
                "-Cdebuginfo=2",
                "-Cpanic=abort",
                "-Crelocation-model=static",
                "-Clink-arg=-nostartfiles",
                "-Clink-arg=-no-pie",
            ])
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report = temp.join(format!("{name}-report"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_fourgate"));
        command
            .arg("measure-panics")
            .arg("--binary")
            .arg(&binary)
            .arg("--output")
            .arg(&report);
        if let Some(tools) = std::env::var_os("FOURGATE_LLVM_TOOLS") {
            command.arg("--llvm-tools").arg(tools);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let inventory: Value =
            serde_json::from_slice(&fs::read(report.join("panics.json")).unwrap()).unwrap();
        let actual = reviewable(&inventory, &source);
        let expected: Value =
            serde_json::from_slice(&fs::read(fixtures.join(format!("{name}.json"))).unwrap())
                .unwrap();
        assert_eq!(
            actual,
            expected,
            "{name}: code -> expected JSON changed\nactual:\n{}",
            serde_json::to_string_pretty(&actual).unwrap()
        );
    }
    fs::remove_dir_all(temp).unwrap();
}
