//! Schema for `errors.json`, the error report for one recorded `fourgate flux` run.
//!
//! The parser reads `stdout.log` and `stderr.log` for diagnostics, ICEs,
//! and panics, and `result.json` for the command's exit or launch failure. Errors
//! retain references into those original files; warnings stay in the raw logs.
//!
//! `fourgate flux` generates this report automatically after recording the command.
//! Malformed diagnostics panic during extraction; the original files remain intact.

#[path = "diagnostics/parser.rs"]
mod parser;
pub use parser::read_report;

use serde::{Deserialize, Serialize};

/// Recovered errors and run completeness, without duplicating the exit code in
/// `result.json`. A normal verification failure can have a complete error report.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorReport {
    /// Version of this report's JSON format.
    pub schema_version: u32,
    /// True iff incomplete_reasons is empty. Does not imply proof coverage,
    /// that every workspace target was checked, or that verification passed.
    pub complete: bool,
    /// Why the run could not produce a complete diagnostic inventory.
    pub incomplete_reasons: Vec<IncompleteReason>,
    /// One entry per occurrence, without warnings or inferred culprit functions.
    pub errors: Vec<FluxError>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompleteReason {
    /// An explicit ICE or panic means compiler diagnostics may be missing.
    CompilerCrash,
    /// Final result metadata is absent, or the child terminated without an exit code.
    RunInterrupted,
    /// The checker command could not start; its error is recorded in `result.json`.
    InvocationFailure,
}

/// One error occurrence. This deliberately avoids deciding whether a diagnostic
/// is a proof failure, unsupported feature, or another compiler/build error.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FluxError {
    pub kind: ErrorKind,
    pub message: String,
    /// Original diagnostic code, if supplied.
    pub code: Option<String>,
    /// Names supplied by Cargo; null when attribution is unknown.
    pub package: Option<String>,
    pub target: Option<String>,
    /// Diagnostic-provided spans only; empty when unavailable.
    pub spans: Vec<Span>,
    /// Nonempty. Merge JSON and text evidence only when confidently associated.
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Diagnostic,
    /// An explicit `internal compiler error` diagnostic. May also have a
    /// matching panic block; retain both as evidence of this occurrence.
    InternalCompilerError,
    /// A panic without an associated explicit ICE diagnostic. This describes
    /// the available evidence, not whether an ICE diagnostic was lost.
    Panic,
    InvocationFailure,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Path exactly as reported, usually relative to the invocation's cwd.
    pub file: String,
    /// One-based coordinates; end is exclusive, matching rustc diagnostics.
    pub line_start: u32,
    pub column_start: u32,
    pub line_end: u32,
    pub column_end: u32,
    pub primary: bool,
    pub label: Option<String>,
}

/// A range in an original recording file, not in the generated error report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub file: EvidenceFile,
    /// One-based, inclusive lines in the original recording, before filtering.
    pub line_start: u32,
    pub line_end: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceFile {
    #[serde(rename = "stdout.log")]
    Stdout,
    #[serde(rename = "stderr.log")]
    Stderr,
    /// Recorder metadata supplies launch errors when no checker output exists.
    #[serde(rename = "result.json")]
    Result,
}
