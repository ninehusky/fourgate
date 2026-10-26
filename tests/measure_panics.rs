//! Run against actual compiled machine code, not synthetic diagnostic logs.
//! Requires Linux and LLVM tools on PATH (or FOURGATE_LLVM_TOOLS).
use std::{fs, path::PathBuf, process::Command};

#[test]
#[ignore = "requires Linux rustc and llvm-objdump/llvm-symbolizer; see stages/measure-panics.md"]
fn inventories_real_panic_sites() {
    assert_eq!(
        std::env::consts::OS,
        "linux",
        "the fixture must produce ELF"
    );
    let temp = std::env::temp_dir().join(format!("fourgate-inventory-{}", std::process::id()));
    fs::create_dir(&temp).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/panic_sites.rs");
    let binary = temp.join("fixture");
    let status = Command::new("rustc")
        .arg(fixture)
        .args([
            "--edition=2024",
            "-Copt-level=3",
            "-Cdebuginfo=2",
            "-Cpanic=abort",
        ])
        .arg("-o")
        .arg(&binary)
        .status()
        .unwrap();
    assert!(status.success());
    let report = temp.join("report");
    let mut command = Command::new(env!("CARGO_BIN_EXE_fourgate"));
    command
        .args(["measure-panics", "--binary"])
        .arg(&binary)
        .arg("--output")
        .arg(&report);
    if let Some(tools) = std::env::var_os("FOURGATE_LLVM_TOOLS") {
        command.arg("--llvm-tools").arg(tools);
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(report.join("panics.json")).unwrap()).unwrap();
    let sites = inventory["sites"].as_array().unwrap();
    for (name, kind) in [
        ("fixture_bounds", "bounds_check"),
        ("fixture_unwrap", "unwrap_or_expect"),
        ("fixture_divide", "arithmetic"),
        ("fixture_copy", "slice_length_mismatch"),
        ("fixture_explicit", "panic_message"),
    ] {
        assert!(
            sites.iter().any(|site| {
                site["kind"] == kind
                    && site["attribution"]["frames"]
                        .as_array()
                        .is_some_and(|frames| {
                            frames.iter().any(|frame| {
                                frame["function"].as_str().is_some_and(|f| f.contains(name))
                            })
                        })
            }),
            "missing {name} / {kind}"
        );
    }
    let asm = fs::read_to_string(report.join("disassembly.asm")).unwrap();
    for site in sites {
        let line = site["assembly_line"].as_u64().unwrap() as usize;
        let text = asm.lines().nth(line - 1).unwrap();
        assert!(text.contains(site["instruction"].as_str().unwrap()));
    }
    assert!(report.join("metadata.json").is_file());
    assert!(report.join("summary.md").is_file());
    // Existing evidence must survive an accidental rerun.
    let original = fs::read(report.join("panics.json")).unwrap();
    assert!(!command.output().unwrap().status.success());
    assert_eq!(original, fs::read(report.join("panics.json")).unwrap());
    fs::remove_dir_all(temp).unwrap();
}
