//! Draft issue ledger API for review. Not yet connected to the CLI.

use serde::{Deserialize, Serialize};
use std::{io, path::Path, path::PathBuf};

/// Proposed minimum fields; the schema is not settled.
#[derive(Debug, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub description: String,
    pub status: IssueStatus,
    pub evidence: Vec<IssueEvidence>,
}

/// An individual piece of evidence associated with an issue.
/// One common example is a snippet of a log file that demonstrates the issue.
/// Unlike [`crate::diagnostics::Evidence`], this can reference files across saved attempts.
#[derive(Debug, Serialize, Deserialize)]
pub struct IssueEvidence {
    pub path: PathBuf, // Relative to the ledger's run directory.
    pub line_start: u32, // One-based, inclusive.
    pub line_end: u32,   // Inclusive.
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    NotStarted,
    InProgress,
    // The issue has been resolved, with an optional resolution note to help
    // the next "person" reviewing the issue understand the resolution.
    // are adding quotes here a microaggression against LLMs? hm...
    Resolved(Option<String>),
    NeedsHuman,
}

/// File an issue in the run's ledger.
///
/// TODO: Settle the schema, ID allocation, duplicate handling, and atomic writes.
pub fn file_issue(_run_directory: &Path, _issue: Issue) -> io::Result<()> {
    todo!("implement after schema review")
}
