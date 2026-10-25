use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn records_failure_and_refuses_overwrite() {
    let root = std::env::temp_dir().join(format!(
        "fourgate-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let task = root.join("attempt");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_fourgate"))
            .args([
                "record",
                env!("CARGO_MANIFEST_DIR"),
                task.to_str().unwrap(),
                "--",
                "sh",
                "-c",
                "echo observed; echo failure >&2; exit 7",
            ])
            .output()
            .unwrap()
    };
    assert_eq!(run().status.code(), Some(7));
    let invocation: serde_json::Value =
        serde_json::from_slice(&fs::read(task.join("invocation.json")).unwrap()).unwrap();
    assert_eq!(invocation["argv"][0], "sh");
    assert_eq!(
        fs::read_to_string(task.join("stdout.log")).unwrap(),
        "observed\n"
    );
    assert_eq!(
        fs::read_to_string(task.join("stderr.log")).unwrap(),
        "failure\n"
    );
    let events = fs::read_to_string(task.join("events.jsonl")).unwrap();
    let events: Vec<serde_json::Value> = events
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(events[1]["exit_code"], 7);
    assert!(!run().status.success());
    assert_eq!(
        fs::read_to_string(task.join("stdout.log")).unwrap(),
        "observed\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn records_launch_error() {
    let root = std::env::temp_dir().join(format!(
        "fourgate-launch-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let task = root.join("attempt");
    let result = Command::new(env!("CARGO_BIN_EXE_fourgate"))
        .args([
            "record",
            env!("CARGO_MANIFEST_DIR"),
            task.to_str().unwrap(),
            "--",
            "/nonexistent-fourgate-test-command",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    let events = fs::read_to_string(task.join("events.jsonl")).unwrap();
    let finish: serde_json::Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
    assert!(finish["exit_code"].is_null());
    assert!(finish["launch_error"].is_string());
    fs::remove_dir_all(root).unwrap();
}
