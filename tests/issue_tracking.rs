use serde_json::{Value, json};
use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Case {
    root: PathBuf,
    repo: PathBuf,
    run: PathBuf,
}
impl Case {
    fn new() -> Self {
        let root = env::temp_dir().join(format!(
            "fourgate-tracking-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let repo = root.join("target");
        let run = root.join("run");
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::create_dir_all(&run).unwrap();
        let out = Command::new("git")
            .args(["init", "--quiet"])
            .arg(&repo)
            .output()
            .unwrap();
        assert!(out.status.success());
        Self { root, repo, run }
    }
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_fourgate"))
            .args(args)
            .output()
            .unwrap()
    }
    fn source(&self, text: &str) {
        fs::write(self.repo.join("src/lib.rs"), text).unwrap();
    }
    fn record(&self, fixture: &str) {
        let dir = self.run.join("baseline");
        fs::create_dir(&dir).unwrap();
        for file in ["stdout.log", "stderr.log", "result.json"] {
            fs::copy(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/flux")
                    .join(fixture)
                    .join(file),
                dir.join(file),
            )
            .unwrap();
        }
        let out = self.cli(&["parse-flux", "--input", dir.to_str().unwrap()]);
        ok(&out);
        fs::write(dir.join("errors.json"), out.stdout).unwrap();
    }
    fn file(&self, value: Value) -> Output {
        let path = self.run.join("draft.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        self.cli(&[
            "issues",
            "file",
            "--run",
            self.run.to_str().unwrap(),
            "--input",
            path.to_str().unwrap(),
        ])
    }
    fn ledger(&self) -> Value {
        serde_json::from_slice(&fs::read(self.run.join("issues.json")).unwrap()).unwrap()
    }
    fn gate(&self) -> Output {
        self.cli(&[
            "issues",
            "gate",
            "--run",
            self.run.to_str().unwrap(),
            "--repo",
            self.repo.to_str().unwrap(),
        ])
    }
    fn observe(&self, extra: &[&str]) -> Output {
        let mut args = vec![
            "issues",
            "observe",
            "--run",
            self.run.to_str().unwrap(),
            "--report",
            "baseline/errors.json",
            "--error-index",
            "0",
        ];
        args.extend_from_slice(extra);
        self.cli(&args)
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn fails(out: &Output, message: &str) {
    assert!(!out.status.success(), "unexpected success");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains(message),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn anchor(role: Value) -> Value {
    json!({"file":"src/lib.rs","line":1,"marker":"bounds","role":role})
}
fn issue(role: Value) -> Value {
    json!({"id":"FLUX-017","description":"Bounds obligation","status":"not_started","evidence":[],"flux":{"places":[anchor(role)],"observations":[{"report":"baseline/errors.json","error_index":0}]}})
}
fn obligation() -> Value {
    json!({"kind":"obligation"})
}
fn exemption(kind: &str) -> Value {
    json!({"kind":"exemption","reason":{"kind":kind,"detail":"Recorded and confirmed cause"}})
}

#[test]
fn observe_roundtrips_deduplicates_and_preserves_stable_issue_identity() {
    let c = Case::new();
    c.record("diagnostic");
    c.source("// fourgate:anchor bounds\nfn decode() {}\n");
    let path = c.run.join("anchor.json");
    fs::write(&path, anchor(obligation()).to_string()).unwrap();
    ok(&c.observe(&["--anchor", path.to_str().unwrap()]));
    ok(&c.observe(&["--issue", "issue-1"]));
    let ledger = c.ledger();
    assert_eq!(ledger.as_array().unwrap().len(), 1);
    assert_eq!(
        ledger[0]["flux"]["observations"].as_array().unwrap().len(),
        1
    );
    ok(&c.gate());
    // Comments, renamed functions, and moved files leave marker identity intact.
    fs::remove_file(c.repo.join("src/lib.rs")).unwrap();
    fs::write(
        c.repo.join("src/moved.rs"),
        "// added comment\n// fourgate:anchor bounds\nfn renamed() {}\n",
    )
    .unwrap();
    ok(&c.gate());
    assert_eq!(c.ledger(), ledger); // Gate does not silently update hints/status.
    let md = fs::read_to_string(c.run.join("ISSUES.md")).unwrap();
    assert!(md.contains("bounds") && md.contains("baseline/errors.json"));
}

#[test]
fn open_issues_require_a_current_unique_place() {
    let c = Case::new();
    c.record("diagnostic");
    c.source("fn decode() {}\n");
    ok(&c.file(issue(obligation())));
    fails(&c.gate(), "missing marker");
    c.source("// fourgate:anchor bounds\nfn a() {}\n// fourgate:anchor bounds\nfn b() {}\n");
    fails(&c.gate(), "duplicate source marker");
    c.source("// fourgate:anchor bounds\n");
    fails(&c.gate(), "has no code");
}

#[test]
fn temporary_trusts_fail_and_retained_trusts_require_both_links_and_reasons() {
    let c = Case::new();
    c.record("diagnostic");
    c.source("// fourgate:anchor bounds\n#[flux::trusted]\nfn decode() {}\n");
    ok(&c.file(issue(obligation())));
    fails(&c.gate(), "temporary or unaccounted trust");
    let mut retained = issue(exemption("real_bug"));
    fs::write(
        c.run.join("issues.json"),
        json!([retained.clone()]).to_string(),
    )
    .unwrap();
    fails(&c.gate(), "backlink");
    c.source(
        "// fourgate:anchor bounds\n// fourgate:trust FLUX-017\n#[flux::trusted]\nfn decode() {}\n",
    );
    ok(&c.gate());
    c.source("// fourgate:anchor bounds\n// fourgate:trust FLUX-017\n#[flux::trusted(reason = \"panic\")]\nfn decode() {}\n");
    fails(&c.gate(), "source trust reason disagrees");
    retained["status"] = json!({"resolved":"fixed"});
    fs::write(c.run.join("issues.json"), json!([retained]).to_string()).unwrap();
    fails(&c.gate(), "resolved issue still claims");
}

#[test]
fn crash_trust_requires_an_actual_recorded_crash() {
    let c = Case::new();
    c.record("diagnostic");
    c.source(
        "// fourgate:anchor bounds\n// fourgate:trust FLUX-017\n#[flux::trusted]\nfn decode() {}\n",
    );
    ok(&c.file(issue(exemption("flux_crash"))));
    fails(&c.gate(), "lacks a recorded crash");
    fs::remove_dir_all(c.run.join("baseline")).unwrap();
    c.record("crashes");
    ok(&c.gate());
}

#[test]
fn nested_cfg_and_macro_attributes_cannot_silently_bypass_the_gate() {
    let c = Case::new();
    fs::write(c.run.join("issues.json"), "[]").unwrap();
    for source in [
        "#[cfg_attr(flux, flux::trusted)]\nfn f() {}",
        "macro_rules! m { () => { #[flux::trusted] fn f() {} } }",
        "#![flux::trusted]\nfn f() {}",
    ] {
        c.source(source);
        fails(&c.gate(), "trust needs one adjacent source anchor");
    }
}

#[test]
fn literals_and_block_comments_do_not_create_fake_markers_or_trusts() {
    let c = Case::new();
    c.record("diagnostic");
    c.source("/*\n// fourgate:anchor bounds\n*/\nconst S: &str = r#\"\n// fourgate:anchor bounds\n#[flux::trusted]\n\"#;\n// fourgate:anchor bounds\nfn f() {}\n");
    ok(&c.file(issue(obligation())));
    ok(&c.gate());
}

#[test]
fn altered_reports_invalid_indices_and_missing_raw_evidence_are_rejected() {
    let c = Case::new();
    c.record("diagnostic");
    let before = fs::read(c.run.join("baseline/errors.json")).unwrap();
    let mut altered: Value = serde_json::from_slice(&before).unwrap();
    altered["errors"][0]["message"] = json!("rewritten");
    fs::write(c.run.join("baseline/errors.json"), altered.to_string()).unwrap();
    fails(
        &c.observe(&["--issue", "missing"]),
        "differs from its raw recording",
    );
    assert!(!c.run.join("issues.json").exists());
    fs::write(c.run.join("baseline/errors.json"), before).unwrap();
    let mut bad = issue(obligation());
    bad["flux"]["observations"][0]["error_index"] = json!(99);
    fails(&c.file(bad), "out of range");
    fs::remove_file(c.run.join("baseline/stdout.log")).unwrap();
    assert!(!c.file(issue(obligation())).status.success());
}

#[test]
fn disappearance_never_automatically_resolves_or_removes_an_issue() {
    let c = Case::new();
    c.record("diagnostic");
    c.source("// fourgate:anchor bounds\nfn f() {}\n");
    ok(&c.file(issue(obligation())));
    let before = c.ledger();
    // A later success is a separate observation, not a reason to close old issues.
    fs::rename(c.run.join("baseline"), c.run.join("old")).unwrap();
    c.record("success");
    let mut ledger = before.clone();
    ledger[0]["flux"]["observations"][0]["report"] = json!("old/errors.json");
    fs::write(c.run.join("issues.json"), ledger.to_string()).unwrap();
    ok(&c.gate());
    assert_eq!(c.ledger()[0]["status"], json!("not_started"));
}

#[test]
fn a_missing_ledger_cannot_silently_pass_the_gate() {
    let c = Case::new();
    c.source("fn f() {}\n");
    fails(&c.gate(), "existing issues.json");
}

#[test]
fn a_subdirectory_cannot_silently_narrow_the_source_gate() {
    let c = Case::new();
    fs::write(c.run.join("issues.json"), "[]").unwrap();
    c.source("fn f() {}\n");
    let out = c.cli(&[
        "issues",
        "gate",
        "--run",
        c.run.to_str().unwrap(),
        "--repo",
        c.repo.join("src").to_str().unwrap(),
    ]);
    fails(&out, "Git checkout root");
}

#[test]
fn unsupported_flux_checking_exemptions_fail_even_without_a_trust_attribute() {
    let c = Case::new();
    fs::write(c.run.join("issues.json"), "[]").unwrap();
    for source in [
        "#[flux::ignore] fn f() {}",
        "#[cfg_attr(flux, flux_rs::ignore)] fn f() {}",
        "#[flux::proven_externally] fn f() {}",
    ] {
        c.source(source);
        fails(&c.gate(), "unsupported checking exemption");
    }
    c.source("#[test] #[ignore] fn f() {}\n");
    ok(&c.gate()); // Rust's built-in ignored test is not a Flux exemption.
}

#[test]
fn ordinary_macro_metavariables_and_interpolation_are_accepted() {
    let c = Case::new();
    fs::write(c.run.join("issues.json"), "[]").unwrap();
    c.source(
        r#"
macro_rules! m { ($(#[$m:meta])* $i:item) => { $(#[$m])* $i }; }
fn f() { quote! { #[doc = #d] fn g() {} }; }
"#,
    );
    ok(&c.gate());
    c.source("macro_rules! m { () => { #[flux::trusted(reason = $r)] fn f() {} } }");
    assert!(!c.gate().status.success()); // Literal trust with dynamic reason fails closed.
}

#[test]
fn unstaged_moves_of_tracked_files_keep_markers_resolvable() {
    let c = Case::new();
    c.record("diagnostic");
    c.source("// fourgate:anchor bounds\nfn f() {}\n");
    ok(&c.file(issue(obligation())));
    ok(&Command::new("git")
        .arg("-C")
        .arg(&c.repo)
        .args(["add", "."])
        .output()
        .unwrap());
    fs::rename(c.repo.join("src/lib.rs"), c.repo.join("src/moved.rs")).unwrap();
    ok(&c.gate());
}

#[test]
fn reobserving_a_marker_refreshes_hints_instead_of_duplicating_identity() {
    let c = Case::new();
    c.record("diagnostic");
    c.source("// fourgate:anchor bounds\nfn f() {}\n");
    ok(&c.file(issue(obligation())));
    let path = c.run.join("anchor.json");
    let mut moved = anchor(obligation());
    moved["line"] = json!(5);
    fs::write(&path, moved.to_string()).unwrap();
    ok(&c.observe(&["--issue", "FLUX-017", "--anchor", path.to_str().unwrap()]));
    let ledger = c.ledger();
    assert_eq!(ledger[0]["flux"]["places"].as_array().unwrap().len(), 1);
    assert_eq!(ledger[0]["flux"]["places"][0]["line"], 5);
    ok(&c.gate());
}

#[test]
fn explicit_flux_no_annotations_do_not_claim_exemptions() {
    let c = Case::new();
    fs::write(c.run.join("issues.json"), "[]").unwrap();
    for attribute in [
        "trusted(no)",
        "trusted_impl(no)",
        "trusted_derive(no)",
        "ignore(no)",
        "trusted(no, reason = \"checked here\")",
    ] {
        c.source(&format!("#[flux::{attribute}] fn f() {{}}\n"));
        ok(&c.gate());
    }
}

#[test]
fn default_trust_and_ignore_configuration_cannot_silently_bypass_accounting() {
    let c = Case::new();
    fs::write(c.run.join("issues.json"), "[]").unwrap();
    c.source("fn f() {}\n");
    for key in [
        "default_trusted",
        "default-trusted",
        "default_ignore",
        "default-ignore",
        "include_trusted",
        "include-trusted",
    ] {
        fs::write(c.repo.join("flux.toml"), format!("{key} = true\n")).unwrap();
        fails(&c.gate(), "trust selection configuration");
    }
}

#[test]
fn excluded_nested_repositories_are_reported_instead_of_silently_skipped() {
    let c = Case::new();
    fs::write(c.run.join("issues.json"), "[]").unwrap();
    let nested = c.repo.join("nested");
    fs::create_dir(&nested).unwrap();
    ok(&Command::new("git")
        .args(["init", "--quiet"])
        .arg(&nested)
        .output()
        .unwrap());
    fs::write(nested.join("lib.rs"), "#[flux::trusted] fn f() {}\n").unwrap();
    let out = c.gate();
    ok(&out);
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("not scanned: nested repository/submodule nested/")
    );
}

#[test]
fn configuration_comments_false_flags_and_unrelated_metadata_are_not_exemptions() {
    let c = Case::new();
    fs::write(c.run.join("issues.json"), "[]").unwrap();
    c.source("fn f() {}\n");
    fs::write(c.repo.join("flux.toml"), "# default_trusted = true\ndefault_trusted = false\ndefault_ignore = false\ninclude_trusted = []\n").unwrap();
    fs::write(
        c.repo.join("Cargo.toml"),
        "[package.metadata.unrelated]\ndefault_trusted = true\n",
    )
    .unwrap();
    ok(&c.gate());
    fs::write(
        c.repo.join("Cargo.toml"),
        "[package.metadata.flux]\ndefault_trusted = true\n",
    )
    .unwrap();
    fails(&c.gate(), "trust selection configuration default_trusted");
}
