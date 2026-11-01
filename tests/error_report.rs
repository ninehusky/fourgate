#[path = "../src/diagnostics.rs"]
mod diagnostics;

use diagnostics::{ErrorKind, ErrorReport, EvidenceFile, IncompleteReason};
use serde_json::Value;
use std::{fs, path::Path};

// A truncated diagnostic must stop extraction with a parser panic, rather than
// produce a report that silently omits the damaged error.
#[test]
#[should_panic(expected = "invalid JSON at Stdout:1")]
fn truncated_diagnostic_recording_panics() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/flux/malformed_json");
    // The should_panic attribute checks both the failure and its log location.
    diagnostics::read_report(&fixture).unwrap();
}

// Parse each saved recording without running Flux:
// 1. Compare extracted errors with the hand-written expected.json and its schema.
// 2. Check version and completeness metadata, plus one-based, ordered source spans.
// 3. Ensure each evidence range fits inside its referenced recording file.
// Evidence usually refers to stdout/stderr. For a command that never started,
// the recorder puts the launch error in result.json, so that file is evidence too.
#[test]
fn reports_fit_schema_and_reference_existing_evidence() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/flux");
    for case in [
        "success",
        "diagnostic",
        "crashes",
        "real_crashes",
        "interrupted",
        "launch_failure",
    ] {
        let dir = fixtures.join(case);
        // 1. Load the expected JSON and deserialize it into the Rust schema.
        let expected: Value =
            serde_json::from_slice(&fs::read(dir.join("expected.json")).unwrap()).unwrap();
        let report: ErrorReport = serde_json::from_value(expected.clone()).unwrap();
        // 2. Extraction must recover exactly the errors and evidence in this fixture.
        assert_eq!(diagnostics::read_report(&dir).unwrap(), report, "{case}");
        // 3. Serializing the schema must preserve every expected JSON field.
        assert_eq!(serde_json::to_value(&report).unwrap(), expected, "{case}");
        // These fixtures describe version 1 of the report format.
        assert_eq!(report.schema_version, 1, "{case}");
        // A report is complete exactly when no incomplete-run reasons are present.
        assert_eq!(
            report.complete,
            report.incomplete_reasons.is_empty(),
            "{case}"
        );
        for error in &report.errors {
            if matches!(
                error.kind,
                ErrorKind::InternalCompilerError | ErrorKind::Panic
            ) {
                // Either crash form must mark the run's diagnostics as partial.
                assert!(
                    report
                        .incomplete_reasons
                        .contains(&IncompleteReason::CompilerCrash)
                );
            }
            if error.kind == ErrorKind::InvocationFailure {
                // A command that never started cannot have a complete report.
                assert!(
                    report
                        .incomplete_reasons
                        .contains(&IncompleteReason::InvocationFailure)
                );
            }
            // Every reported error must point back to at least one piece of evidence.
            assert!(!error.evidence.is_empty(), "{case}");
            for span in &error.spans {
                // rustc source coordinates start at one, so zero is invalid here.
                assert!(span.line_start > 0 && span.column_start > 0, "{case}");
                // The exclusive end uses the same one-based coordinate system.
                assert!(span.line_end > 0 && span.column_end > 0, "{case}");
                // A source range cannot end before its start.
                assert!(
                    (span.line_start, span.column_start) <= (span.line_end, span.column_end),
                    "{case}"
                );
            }
            for evidence in &error.evidence {
                let name = match evidence.file {
                    EvidenceFile::Stdout => "stdout.log",
                    EvidenceFile::Stderr => "stderr.log",
                    EvidenceFile::Result => "result.json",
                };
                let text = fs::read_to_string(dir.join(name)).unwrap();
                // Evidence references use one-based lines in the recorded file.
                assert!(evidence.line_start > 0, "{case}");
                // Inclusive evidence ranges must contain at least their start line.
                assert!(evidence.line_end >= evidence.line_start, "{case}");
                // The referenced range must fit inside the actual evidence file.
                assert!(evidence.line_end as usize <= text.lines().count(), "{case}");
            }
        }
    }
}
