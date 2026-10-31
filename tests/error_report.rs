// Compile the draft schema without wiring it into the CLI yet.
#[path = "../src/diagnostics.rs"]
mod diagnostics;

use diagnostics::{ErrorKind, ErrorReport, EvidenceFile, IncompleteReason};
use serde_json::Value;
use std::{fs, path::Path};

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
        let expected: Value =
            serde_json::from_slice(&fs::read(dir.join("expected.json")).unwrap()).unwrap();
        let report: ErrorReport = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(&report).unwrap(), expected, "{case}");
        assert_eq!(report.schema_version, 1, "{case}");
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
                assert!(
                    report
                        .incomplete_reasons
                        .contains(&IncompleteReason::CompilerCrash)
                );
            }
            if error.kind == ErrorKind::InvocationFailure {
                assert!(
                    report
                        .incomplete_reasons
                        .contains(&IncompleteReason::InvocationFailure)
                );
            }
            assert!(!error.evidence.is_empty(), "{case}");
            for span in &error.spans {
                assert!(span.line_start > 0 && span.column_start > 0, "{case}");
                assert!(span.line_end > 0 && span.column_end > 0, "{case}");
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
                assert!(evidence.line_start > 0, "{case}");
                assert!(evidence.line_end >= evidence.line_start, "{case}");
                assert!(evidence.line_end as usize <= text.lines().count(), "{case}");
            }
        }
    }
}
