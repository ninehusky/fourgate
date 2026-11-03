use super::*;
use serde_json::Value;
use std::{fs, io, path::Path};

/// Read evidence without changing it. Missing result metadata means interruption;
/// malformed diagnostics or result metadata panic, preserving the raw evidence.
pub fn read_report(directory: &Path) -> io::Result<ErrorReport> {
    let stdout = fs::read(directory.join("stdout.log"))?;
    let stderr = fs::read(directory.join("stderr.log"))?;
    let result = match fs::read(directory.join("result.json")) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    Ok(parse(&stdout, &stderr, result.as_deref()))
}

#[derive(Deserialize)]
struct Diagnostic {
    level: String,
    message: String,
    code: Option<DiagnosticCode>,
    #[serde(default)]
    spans: Vec<DiagnosticSpan>,
}

#[derive(Deserialize)]
struct DiagnosticCode {
    code: String,
}

#[derive(Deserialize)]
struct DiagnosticSpan {
    file_name: String,
    line_start: u32,
    column_start: u32,
    line_end: u32,
    column_end: u32,
    is_primary: bool,
    label: Option<String>,
}

#[derive(Deserialize)]
struct RecordedResult {
    exit_code: Option<i32>,
    launch_error: Option<String>,
}

struct Panic {
    location: String,
    message: String,
    evidence: Evidence,
}

fn parse(stdout: &[u8], stderr: &[u8], result: Option<&[u8]>) -> ErrorReport {
    let mut errors = Vec::new();
    let mut panics = Vec::new();
    let mut subprocess_interrupted = false;
    for (bytes, file) in [
        (stdout, EvidenceFile::Stdout),
        (stderr, EvidenceFile::Stderr),
    ] {
        let text = std::str::from_utf8(bytes)
            .unwrap_or_else(|error| panic!("invalid UTF-8 in {file:?}: {error}"));
        let lines: Vec<_> = text.lines().map(strip_ansi).collect();
        let mut consumed_until = 0;
        for (index, line) in lines.iter().enumerate() {
            if index < consumed_until {
                continue;
            }
            let trimmed = line.trim();
            // Cargo can exit normally after a compiler subprocess was killed.
            // Its own exit_code alone therefore cannot establish completeness.
            if trimmed.starts_with("process didn't exit successfully:")
                && trimmed.contains("(signal: ")
            {
                subprocess_interrupted = true;
            }
            if trimmed.starts_with('{') {
                let value = serde_json::from_str::<Value>(trimmed).unwrap_or_else(|error| {
                    panic!("invalid JSON at {file:?}:{}: {error}", index + 1)
                });
                if let Some(error) = json_error(&value, file, index + 1) {
                    errors.push(FluxError {
                        evidence: vec![evidence(&file, index, index)],
                        ..error
                    });
                }
            } else if let Some(location) = panic_location(line) {
                let mut end = index;
                let mut message = line.clone();
                if let Some(next) = lines.get(index + 1)
                    && !next.is_empty()
                    && !log_boundary(next)
                    && !backtrace_line(next)
                {
                    message = next.clone();
                    end += 1;
                }
                while lines.get(end + 1).is_some_and(|s| backtrace_line(s)) {
                    end += 1;
                }
                panics.push(Panic {
                    location: location.to_owned(),
                    message,
                    evidence: evidence(&file, index, end),
                });
                consumed_until = end + 1;
            } else if let Some((kind, message, code)) = text_error(trimmed) {
                errors.push(FluxError {
                    kind,
                    message,
                    code,
                    package: None,
                    target: None,
                    spans: vec![],
                    evidence: vec![evidence(&file, index, index)],
                });
            }
        }
    }

    // File/line/column alone is not a process identifier. Associate only when
    // exactly one ICE and one panic share that compiler location in this run.
    for panic in &panics {
        let candidates: Vec<_> = errors
            .iter()
            .enumerate()
            .filter(|(_, error)| {
                error.kind == ErrorKind::InternalCompilerError
                    && error.message.starts_with(&format!("{}:", panic.location))
            })
            .map(|(index, _)| index)
            .collect();
        let unique_panic = panics
            .iter()
            .filter(|p| p.location == panic.location)
            .count()
            == 1;
        if candidates.len() == 1 && unique_panic {
            errors[candidates[0]].evidence.push(panic.evidence);
        } else {
            errors.push(FluxError {
                kind: ErrorKind::Panic,
                message: panic.message.clone(),
                code: None,
                package: None,
                target: None,
                spans: vec![],
                evidence: vec![panic.evidence],
            });
        }
    }

    let (interrupted, invocation_failed) = match result {
        None => (true, false),
        Some(bytes) => {
            let value: Value = serde_json::from_slice(bytes).expect("invalid JSON in result.json");
            assert!(
                value.get("exit_code").is_some() && value.get("launch_error").is_some(),
                "missing exit_code or launch_error in result.json"
            );
            let parsed: RecordedResult =
                serde_json::from_value(value).expect("invalid fields in result.json");
            match parsed {
                RecordedResult {
                    exit_code,
                    launch_error: Some(message),
                } => {
                    assert!(
                        exit_code.is_none(),
                        "launch error with exit code in result.json"
                    );
                    let text = std::str::from_utf8(bytes).expect("invalid UTF-8 in result.json");
                    let index = text
                        .lines()
                        .position(|s| s.contains("\"launch_error\""))
                        .unwrap_or(0);
                    errors.push(FluxError {
                        kind: ErrorKind::InvocationFailure,
                        message,
                        code: None,
                        package: None,
                        target: None,
                        spans: vec![],
                        evidence: vec![evidence(&EvidenceFile::Result, index, index)],
                    });
                    (false, true)
                }
                RecordedResult {
                    exit_code,
                    launch_error: None,
                } => {
                    assert!(
                        !exit_code.is_some_and(|code| code != 0)
                            || !errors.is_empty()
                            || subprocess_interrupted,
                        "failed command without a recovered error"
                    );
                    (exit_code.is_none(), false)
                }
            }
        }
    };
    let crashed = errors.iter().any(|error| {
        matches!(
            error.kind,
            ErrorKind::InternalCompilerError | ErrorKind::Panic
        )
    });
    let incomplete_reasons: Vec<_> = [
        (crashed, IncompleteReason::CompilerCrash),
        (
            interrupted || subprocess_interrupted,
            IncompleteReason::RunInterrupted,
        ),
        (invocation_failed, IncompleteReason::InvocationFailure),
    ]
    .into_iter()
    .filter_map(|(applies, reason)| applies.then_some(reason))
    .collect();
    ErrorReport {
        schema_version: 1,
        complete: incomplete_reasons.is_empty(),
        incomplete_reasons,
        errors,
    }
}

fn json_error(value: &Value, file: EvidenceFile, line: usize) -> Option<FluxError> {
    let message = if value.get("reason").and_then(Value::as_str) == Some("compiler-message") {
        value.get("message").unwrap_or(&Value::Null)
    } else if value.get("$message_type").and_then(Value::as_str) == Some("diagnostic") {
        value
    } else {
        return None;
    };
    let diagnostic: Diagnostic = serde_json::from_value(message.clone())
        .unwrap_or_else(|error| panic!("invalid compiler diagnostic at {file:?}:{line}: {error}"));
    let kind = if diagnostic.level == "error: internal compiler error"
        || (diagnostic.level == "error"
            && (diagnostic.message == "internal compiler error"
                || diagnostic.message.starts_with("internal compiler error:")))
    {
        ErrorKind::InternalCompilerError
    } else if diagnostic.level == "error" {
        ErrorKind::Diagnostic
    } else {
        return None;
    };
    let mut spans = Vec::new();
    for span in diagnostic.spans {
        assert!(
            span.line_start > 0
                && span.column_start > 0
                && span.line_end > 0
                && span.column_end > 0
                && (span.line_end, span.column_end) >= (span.line_start, span.column_start),
            "invalid diagnostic span at {file:?}:{line}"
        );
        spans.push(Span {
            file: span.file_name,
            line_start: span.line_start,
            column_start: span.column_start,
            line_end: span.line_end,
            column_end: span.column_end,
            primary: span.is_primary,
            label: span.label,
        });
    }
    Some(FluxError {
        kind,
        message: diagnostic
            .message
            .strip_prefix("internal compiler error: ")
            .unwrap_or(&diagnostic.message)
            .to_owned(),
        code: diagnostic.code.map(|code| code.code),
        package: value
            .get("package_id")
            .and_then(Value::as_str)
            .and_then(package_name),
        target: value
            .pointer("/target/name")
            .and_then(Value::as_str)
            .map(str::to_owned),
        spans,
        evidence: vec![],
    })
}

fn package_name(id: &str) -> Option<String> {
    if let Some((source, fragment)) = id.rsplit_once('#') {
        if let Some((name, _)) = fragment.split_once('@') {
            return Some(name.to_owned());
        }
        return source
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
    }
    // Older Cargo: `name version (registry+...)`.
    id.split_whitespace().next().map(str::to_owned)
}

fn panic_location(line: &str) -> Option<&str> {
    line.strip_prefix("thread ")?
        .split_once(" panicked at ")
        .map(|(_, location)| location.trim_end_matches(':'))
}

fn log_boundary(line: &str) -> bool {
    line.trim_start().starts_with('{')
        || line.starts_with("FLUX ")
        || line.starts_with("error")
        || line.starts_with("warning")
        || line.starts_with("note:")
        || panic_location(line).is_some()
        || line.trim_start().starts_with("Checking ")
        || line.trim_start().starts_with("Compiling ")
}

fn backtrace_line(line: &str) -> bool {
    line == "stack backtrace:"
        || (line.starts_with(char::is_whitespace)
            && (line.trim_start().starts_with("at ")
                || line
                    .trim_start()
                    .split_once(':')
                    .is_some_and(|(n, _)| n.parse::<usize>().is_ok())))
}

fn text_error(line: &str) -> Option<(ErrorKind, String, Option<String>)> {
    if line == "error: internal compiler error" {
        return Some((
            ErrorKind::InternalCompilerError,
            "internal compiler error".to_owned(),
            None,
        ));
    }
    if let Some(message) = line.strip_prefix("error: internal compiler error: ") {
        return Some((ErrorKind::InternalCompilerError, message.to_owned(), None));
    }
    if let Some(message) = line.strip_prefix("error: ") {
        if message.starts_with("could not compile ") || message.starts_with("aborting due to ") {
            return None;
        }
        return Some((ErrorKind::Diagnostic, message.to_owned(), None));
    }
    if let Some(rest) = line.strip_prefix("error[")
        && let Some((code, message)) = rest.split_once("]: ")
    {
        return Some((
            ErrorKind::Diagnostic,
            message.to_owned(),
            Some(code.to_owned()),
        ));
    }
    if line.starts_with("Failed to run `cargo-flux`, error=") {
        return Some((ErrorKind::Diagnostic, line.to_owned(), None));
    }
    None
}

fn evidence(file: &EvidenceFile, start: usize, end: usize) -> Evidence {
    Evidence {
        file: *file,
        line_start: start as u32 + 1,
        line_end: end as u32 + 1,
    }
}

fn strip_ansi(line: &str) -> String {
    let mut chars = line.chars().peekable();
    let mut clean = String::new();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        } else {
            clean.push(c);
        }
    }
    clean
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SUCCESS: &[u8] = br#"{"exit_code":0,"launch_error":null}"#;
    const FAILURE: &[u8] = br#"{"exit_code":101,"launch_error":null}"#;

    #[test]
    fn cargo_subprocess_signal_is_an_interruption_even_with_other_diagnostics() {
        let killed = b"Caused by:\n  process didn't exit successfully: `flux-driver ...` (signal: 9, SIGKILL: kill)\n";
        for stdout in [
            b"".as_slice(),
            b"error: ordinary proof failure\n".as_slice(),
        ] {
            let report = parse(stdout, killed, Some(FAILURE));
            assert!(!report.complete);
            assert_eq!(
                report.incomplete_reasons,
                vec![IncompleteReason::RunInterrupted]
            );
        }
        // Quoting Cargo's wording in a diagnostic is not a subprocess signal.
        let report = parse(
            b"error: process didn't exit successfully: (signal: 9)\n",
            b"",
            Some(FAILURE),
        );
        assert!(report.complete);
    }

    // A proof diagnostic can mention panic text without reporting an actual
    // compiler crash. Preserve its code and stream attribution as a diagnostic.
    #[test]
    fn distinguishes_proof_diagnostics_from_actual_panics() {
        let json = json!({
            "$message_type": "diagnostic", "level": "error",
            "message": "call may panic: thread 'rustc' panicked at example.rs:1:1:",
            "code": {"code": "FLUX"}, "spans": []
        });
        let report = parse(b"", json.to_string().as_bytes(), Some(FAILURE));
        // A normal verification failure does not make extraction incomplete.
        assert!(report.complete);
        // The embedded panic wording must not create an additional error.
        assert_eq!(report.errors.len(), 1);
        // Classification must come from the diagnostic level, not message keywords.
        assert_eq!(report.errors[0].kind, ErrorKind::Diagnostic);
        // Preserve the supplied diagnostic code for the report's consumer.
        assert_eq!(report.errors[0].code.as_deref(), Some("FLUX"));
        // Rustc JSON on stderr must still point to stderr as its evidence.
        assert_eq!(report.errors[0].evidence[0].file, EvidenceFile::Stderr);
    }

    // Two processes panicking at one compiler location are ambiguous. Keep both
    // occurrences separate from the ICE and do not infer culprits from nearby logs.
    #[test]
    fn ambiguous_processes_are_not_merged_or_assigned_nearby_functions() {
        let json = json!({
            "$message_type": "diagnostic", "level": "error: internal compiler error",
            "message": "compiler.rs:1:2: impossible case", "spans": []
        });
        let stderr = b"FLUX checking: unrelated::function\n\
            thread 'rustc' (1) panicked at compiler.rs:1:2:\nBox<dyn Any>\n\n\
            thread 'rustc' (2) panicked at compiler.rs:1:2:\nBox<dyn Any>\n";
        let report = parse(json.to_string().as_bytes(), stderr, Some(FAILURE));
        // The ambiguous ICE and two panic occurrences must all remain visible.
        assert_eq!(report.errors.len(), 3);
        // The explicit ICE must retain its original classification.
        assert_eq!(report.errors[0].kind, ErrorKind::InternalCompilerError);
        // Neither ambiguous panic block may be attached to the ICE's evidence.
        assert_eq!(report.errors[0].evidence.len(), 1);
        for error in &report.errors[1..] {
            // Each unmatched block remains a plain panic occurrence.
            assert_eq!(error.kind, ErrorKind::Panic);
            // Nearby verbose output does not establish a package for these panics.
            assert_eq!(error.package, None);
            // A compiler panic location is not a diagnostic-provided source span.
            assert!(error.spans.is_empty());
        }
    }

    // Read a colored text diagnostic after a warning, keeping the original raw
    // line number while omitting warning output and terminal color escapes.
    #[test]
    fn text_errors_keep_original_line_numbers_and_ignore_color() {
        let report = parse(
            b"",
            b"warning: unused item\n\x1b[31merror[E0308]: mismatched types\x1b[0m\n",
            Some(FAILURE),
        );
        // A recognized diagnostic is fully extracted despite the nonzero exit.
        assert!(report.complete);
        // The warning must not become an error entry.
        assert_eq!(report.errors.len(), 1);
        // Color escapes and the error prefix must not pollute the stored message.
        assert_eq!(report.errors[0].message, "mismatched types");
        // Preserve the Rust error code from the text header.
        assert_eq!(report.errors[0].code.as_deref(), Some("E0308"));
        // Filtering out the preceding warning must not renumber the evidence.
        assert_eq!(report.errors[0].evidence[0].line_start, 2);
    }

    // An explicit text ICE needs no additional message or backtrace to establish
    // that the compiler crashed.
    #[test]
    fn bare_text_ice_is_still_a_compiler_crash() {
        let report = parse(b"", b"error: internal compiler error\n", Some(FAILURE));
        // An ICE always makes the diagnostic inventory incomplete.
        assert_eq!(
            report.incomplete_reasons,
            vec![IncompleteReason::CompilerCrash]
        );
        // A bare ICE header must not fall back to a normal diagnostic.
        assert_eq!(report.errors[0].kind, ErrorKind::InternalCompilerError);
    }

    // Concurrent output can put an independent diagnostic immediately after a
    // panic header. Do not consume that diagnostic as the panic's message.
    #[test]
    fn panic_does_not_swallow_a_concurrent_diagnostic() {
        let report = parse(
            b"",
            b"thread 'rustc' panicked at solver.rs:1:1:\nerror[E0001]: concurrent diagnostic\n",
            Some(FAILURE),
        );
        // Both independently recognizable errors must survive extraction.
        assert_eq!(report.errors.len(), 2);
        // The text error retains its own classification outside the panic block.
        assert_eq!(report.errors[0].kind, ErrorKind::Diagnostic);
        // The unmatched header still records a panic without an inferred message.
        assert_eq!(report.errors[1].kind, ErrorKind::Panic);
    }

    // Missing final metadata and a recorded signal exit are legitimate interrupted
    // runs when the logs are well-formed, rather than parser failures.
    #[test]
    fn interrupted_runs_remain_reportable() {
        // No result.json means the recorder did not finish saving a final result.
        assert_eq!(
            parse(b"", b"", None).incomplete_reasons,
            vec![IncompleteReason::RunInterrupted]
        );
        // A null exit code without a launch error represents a signaled child.
        assert_eq!(
            parse(b"", b"", Some(br#"{"exit_code":null,"launch_error":null}"#)).incomplete_reasons,
            vec![IncompleteReason::RunInterrupted]
        );
    }

    // A successful command may emit no diagnostics; an empty report is valid then.
    #[test]
    fn successful_empty_output_is_complete() {
        // No output and a successful recorded exit provide no incomplete-run reason.
        assert!(parse(b"", b"", Some(SUCCESS)).complete);
    }

    // Valid JSON with missing recorder fields is corrupt metadata. The expected
    // panic verifies it cannot be mistaken for an ordinary interruption.
    #[test]
    #[should_panic(expected = "missing exit_code or launch_error in result.json")]
    fn malformed_result_panics() {
        parse(b"", b"", Some(b"{}"));
    }

    // Truncated result JSON must stop parsing with a panic naming the damaged file.
    #[test]
    #[should_panic(expected = "invalid JSON in result.json")]
    fn truncated_result_panics() {
        parse(b"", b"", Some(b"{"));
    }

    // A failed command with no recognizable error means extraction missed its
    // cause. Panic rather than return an apparently valid empty error list.
    #[test]
    #[should_panic(expected = "failed command without a recovered error")]
    fn unrecovered_failure_panics() {
        parse(b"", b"unrecognized launcher failure", Some(FAILURE));
    }

    // Invalid log encoding must fail loudly rather than replace bytes and silently
    // change the evidence used to interpret a diagnostic.
    #[test]
    #[should_panic(expected = "invalid UTF-8 in Stdout")]
    fn invalid_utf8_panics() {
        parse(b"\xff", b"", Some(SUCCESS));
    }

    // A malformed diagnostic must stop extraction even when a later diagnostic
    // is readable; the later entry cannot justify publishing a partial report.
    #[test]
    #[should_panic(expected = "invalid compiler diagnostic at Stdout:1")]
    fn malformed_diagnostic_panics_instead_of_returning_partial_errors() {
        let stdout = b"{\"reason\":\"compiler-message\"}\n\
            {\"$message_type\":\"diagnostic\",\"level\":\"error\",\"message\":\"still recovered\"}\n";
        parse(stdout, b"", Some(FAILURE));
    }

    // A zero source line violates rustc's one-based coordinates. The expected
    // panic checks that extraction does not quietly discard the invalid span.
    #[test]
    #[should_panic(expected = "invalid diagnostic span at Stdout:1")]
    fn invalid_span_panics() {
        let diagnostic = json!({
            "$message_type": "diagnostic", "level": "error", "message": "example",
            "spans": [{"file_name": "src/lib.rs", "line_start": 0, "column_start": 1,
                       "line_end": 1, "column_end": 2, "is_primary": true}]
        });
        parse(diagnostic.to_string().as_bytes(), b"", Some(FAILURE));
    }

    // Cargo package IDs have several forms, and a package name may differ from
    // its directory name. Recover the package name from each supported form.
    #[test]
    fn cargo_package_ids_preserve_package_names() {
        for (id, expected) in [
            (
                "path+file:///work/acpi-tables#acpi_tables@0.2.1",
                "acpi_tables",
            ),
            ("path+file:///work/clippy-tracing#0.1.0", "clippy-tracing"),
            ("registry+https://index#serde@1.0.0", "serde"),
            ("serde 1.0.0 (registry+https://index)", "serde"),
        ] {
            // Preserve the package's identity rather than assuming a target or folder name.
            assert_eq!(package_name(id).as_deref(), Some(expected));
        }
    }
}
