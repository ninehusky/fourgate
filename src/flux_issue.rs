//! Persistent source/evidence tracking for Flux obligations and retained trusts.
//!
//! One ledger describes one target checkout; repository identity belongs to the
//! ledger's context, not every source location. Existing Issue supplies persistent
//! identity, description, status, and log evidence. FluxError remains a recorded
//! diagnostic occurrence, referenced rather than copied here.
//!
//! Consistency gate: unresolved issues have a resolving source anchor; every trust
//! links to an unresolved issue and matches its recorded reason; observation
//! references resolve. Marker maintenance after rewrites remains a human/agent job.
//!
//! Stored as optional `Issue.flux` metadata in the existing issue ledger.
//! Code-linked crash follow-ups may also carry this metadata; observations retain
//! their recorded ErrorKind rather than pretending a crash is a refinement error.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Source/evidence tracking for a Flux obligation in the shared issue ledger.
/// The enclosing Issue owns identity, description, and status.
#[derive(Debug, Serialize, Deserialize)]
pub struct FluxRefinementIssue {
    /// Current code locations; unresolved issues must have at least one.
    pub places: Vec<SourceAnchor>,
    /// Recorded errors supporting this obligation. Diagnostic kind alone does
    /// not establish a real program bug or even identify a refinement error.
    /// Disappearance in a later run does not resolve this issue.
    pub observations: Vec<FluxErrorRef>,
}

/// A maintained location in the target checkout's Rust source.
/// Configuration-based/generated trust is not supported by the initial gate.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAnchor {
    /// File relative to the target checkout, without parent traversal.
    pub file: PathBuf,
    /// One-based navigation hint. A shifted line does not change identity.
    pub line: u32,
    /// Token matching [a-z0-9]+(-[a-z0-9]+)*, unique within the checkout.
    /// Source marker is an entire comment line: `// fourgate:anchor <token>`
    /// (leading indentation allowed), immediately before the relevant code or
    /// its attributes, with only blank/comment lines between them.
    /// For example: `// fourgate:anchor packet-bounds`.
    /// Resolve by marker, searching the checkout if the file hint is stale.
    pub marker: String,
    pub role: AnchorRole,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnchorRole {
    /// Code carrying the unresolved refinement obligation.
    Obligation,
    /// Retained trust annotation at the anchored location.
    /// Its backlink is an exact `// fourgate:trust <issue-id>` comment, placed
    /// between the anchor comment and the trust annotation.
    /// The issue description explains the exemption. If a source reason argument
    /// is present, its category must agree: real_bug for RealBug, panic for FluxCrash.
    Exemption {
        /// Accepted reason for retaining this trust in committed target code.
        /// Temporary inventory trusts have no accepted reason and must be removed.
        reason: TrustReason,
    },
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum TrustReason {
    /// Confirmed bug requiring a maintainer fix; a failed proof alone is insufficient.
    RealBug(String),
    /// Reproduced checker ICE/panic, supported by recorded evidence.
    FluxCrash(String),
}

/// One occurrence in a saved diagnostics::ErrorReport (`errors.json`).
/// Recorded attempts must be retained without rewriting or reordering reports.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FluxErrorRef {
    /// Path to errors.json, relative to the ledger directory. The adjacent run
    /// files retain invocation, source revision, and original diagnostic logs.
    pub report: PathBuf,
    /// Zero-based index into that report's errors array; not a persistent issue id.
    pub error_index: usize,
}
