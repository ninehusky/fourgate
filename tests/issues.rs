use serde_json::{Value, json};
use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Run(PathBuf);
impl Run {
    fn new() -> Self {
        let path = env::temp_dir().join(format!(
            "fourgate-issues-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn command(&self, action: &str, issue: Option<&Value>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_fourgate"));
        command.args(["issues", action, "--run"]).arg(&self.0);
        if let Some(issue) = issue {
            let input = self.0.join("input.json");
            fs::write(&input, serde_json::to_vec(issue).unwrap()).unwrap();
            command.arg("--input").arg(input);
        }
        command.output().unwrap()
    }
    fn ledger(&self) -> Value {
        serde_json::from_slice(&fs::read(self.0.join("issues.json")).unwrap()).unwrap()
    }
}
impl Drop for Run {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn issue() -> Value {
    json!({"id":"", "description":"Isolate compiler crash", "status":"not_started", "evidence":[]})
}
fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn files_updates_and_rebuilds_a_review_table_without_losing_evidence() {
    let run = Run::new();
    fs::create_dir(run.0.join("baseline")).unwrap();
    fs::write(
        run.0.join("baseline/crash log.txt"),
        "header\nthread rustc panicked\nlocation\n",
    )
    .unwrap();
    let mut first = issue();
    first["evidence"] = json!([{"path":"baseline/crash log.txt", "line_start":2, "line_end":3}]);
    success(run.command("file", Some(&first)));
    success(run.command("file", Some(&issue())));
    assert_eq!(run.ledger()[0]["id"], "issue-1");
    assert_eq!(run.ledger()[1]["id"], "issue-2");
    let mut update = run.ledger()[0].clone();
    update["description"] = json!("Trust candidate a | b\nReview <body> [link](bad)");
    update["status"] = json!("needs_human");
    success(run.command("update", Some(&update)));
    assert_eq!(run.ledger()[0]["evidence"], first["evidence"]);
    let table = fs::read_to_string(run.0.join("ISSUES.md")).unwrap();
    assert!(table.contains("a &#124; b<br>Review &lt;body&gt; &#91;link&#93;(bad)"));
    assert!(table.contains("NeedsHuman"));
    assert!(table.contains("baseline/crash%20log.txt#L2-L3"));
    update["status"] = json!({"resolved":"Accepted after review"});
    success(run.command("update", Some(&update)));
    fs::write(run.0.join("ISSUES.md"), "stale").unwrap();
    success(run.command("list", None));
    assert!(
        fs::read_to_string(run.0.join("ISSUES.md"))
            .unwrap()
            .contains("Resolved: Accepted after review")
    );
}

#[test]
fn rejects_invalid_changes_and_preserves_existing_ledger_and_view() {
    let run = Run::new();
    success(run.command("file", Some(&issue())));
    let json_before = fs::read(run.0.join("issues.json")).unwrap();
    let table_before = fs::read(run.0.join("ISSUES.md")).unwrap();
    let mut bad = run.ledger()[0].clone();
    assert!(!run.command("file", Some(&bad)).status.success());
    bad["id"] = json!("unknown");
    assert!(!run.command("update", Some(&bad)).status.success());
    bad["id"] = json!("issue-1");
    for path in ["../outside.log", "/tmp/outside.log", "missing.log"] {
        bad["evidence"] = json!([{"path":path,"line_start":1,"line_end":1}]);
        assert!(!run.command("update", Some(&bad)).status.success());
    }
    fs::write(run.0.join("log.txt"), "one line\n").unwrap();
    for (start, end) in [(0, 1), (2, 1), (1, 2)] {
        bad["evidence"] = json!([{"path":"log.txt","line_start":start,"line_end":end}]);
        assert!(!run.command("update", Some(&bad)).status.success());
    }
    bad["evidence"] = json!([]);
    bad["status"] = json!("made_up");
    assert!(!run.command("update", Some(&bad)).status.success());
    assert_eq!(fs::read(run.0.join("issues.json")).unwrap(), json_before);
    assert_eq!(fs::read(run.0.join("ISSUES.md")).unwrap(), table_before);
    assert!(!run.0.join(".issues.lock").exists());
}

#[test]
fn corrupt_ledger_and_another_writer_cannot_be_overwritten() {
    let run = Run::new();
    fs::write(run.0.join("issues.json"), "{").unwrap();
    assert!(!run.command("file", Some(&issue())).status.success());
    assert_eq!(fs::read_to_string(run.0.join("issues.json")).unwrap(), "{");
    fs::remove_file(run.0.join("issues.json")).unwrap();
    fs::write(run.0.join(".issues.lock"), "other writer").unwrap();
    assert!(!run.command("file", Some(&issue())).status.success());
    assert!(!run.0.join("issues.json").exists());
    assert_eq!(
        fs::read_to_string(run.0.join(".issues.lock")).unwrap(),
        "other writer"
    );
}

#[test]
fn markdown_failure_keeps_committed_json_and_can_be_recovered() {
    let run = Run::new();
    fs::create_dir(run.0.join("ISSUES.md")).unwrap();
    let output = run.command("file", Some(&issue()));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("issues.json saved"));
    assert_eq!(run.ledger()[0]["id"], "issue-1");
    fs::remove_dir(run.0.join("ISSUES.md")).unwrap();
    success(run.command("list", None));
    assert!(
        fs::read_to_string(run.0.join("ISSUES.md"))
            .unwrap()
            .contains("issue-1")
    );
    assert!(!run.0.join("ISSUES.tmp").exists());
}
