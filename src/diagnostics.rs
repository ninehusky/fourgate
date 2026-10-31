//! Draft schema for errors.json. Not yet produced by `fourgate flux`.

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorReport {
    pub schema_version: u32,
    /// True iff incomplete_reasons is empty. Does not imply proof coverage,
    /// that every workspace target was checked, or that verification passed.
    pub complete: bool,
    pub incomplete_reasons: Vec<IncompleteReason>,
    /// One entry per occurrence, without warnings or inferred culprit functions.
    pub errors: Vec<FluxError>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompleteReason {
    CompilerCrash,
    RunInterrupted,
    InvocationFailure,
    ExtractionFailure,
}

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

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub file: EvidenceFile,
    /// One-based, inclusive lines in the original recording, before filtering.
    pub line_start: u32,
    pub line_end: u32,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceFile {
    #[serde(rename = "stdout.log")]
    Stdout,
    #[serde(rename = "stderr.log")]
    Stderr,
    #[serde(rename = "result.json")]
    Result,
}
