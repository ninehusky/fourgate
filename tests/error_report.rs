// Compile the draft schema without wiring it into the CLI yet.
#[path = "../src/diagnostics.rs"]
mod diagnostics;

use diagnostics::{ErrorKind, ErrorReport, EvidenceFile, IncompleteReason};
use serde_json::Value;
use std::{fs, path::Path};

// Check each hand-written report against the Rust schema and verify its status,
// source coordinates, and references into the accompanying raw evidence.
#[test]
fn reports_fit_schema_and_reference_existing_evidence() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/flux");
    for case in [
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
        // 2. Serializing the schema must preserve every expected JSON field.
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
