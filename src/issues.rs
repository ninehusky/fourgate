//! Run-local todos. `issues.json` is authoritative; `ISSUES.md` is a generated view.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

/// A todo, independent of any individual Flux diagnostic.
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
    pub path: PathBuf,   // Relative to the ledger's run directory.
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

/// Manage a run's issue ledger and refresh its Markdown table.
#[derive(Args)]
pub struct Issues {
    #[command(subcommand)]
    action: IssueAction,
}

#[derive(Subcommand)]
enum IssueAction {
    /// File a JSON Issue. Use an empty id to allocate one automatically.
    File(Change),
    /// Replace an existing issue by id with a complete JSON Issue.
    Update(Change),
    /// Print the table and regenerate ISSUES.md from issues.json.
    List {
        #[arg(long)]
        run: PathBuf,
    },
}

#[derive(Args)]
struct Change {
    /// Ledger directory; store immutable checker attempts beneath it.
    #[arg(long)]
    run: PathBuf,
    /// JSON file containing one Issue, including status and evidence.
    #[arg(long)]
    input: PathBuf,
}

pub fn run(args: Issues) -> io::Result<()> {
    match args.action {
        IssueAction::File(args) => {
            let issue = read_issue(&args.input)?;
            println!("Filed {}", file_issue(&args.run, issue)?);
        }
        IssueAction::Update(args) => {
            let issue = read_issue(&args.input)?;
            let id = issue.id.clone();
            update_issue(&args.run, issue)?;
            println!("Updated {id}");
        }
        IssueAction::List { run } => {
            let _lock = LedgerLock::acquire(&run)?;
            let issues = read_ledger(&run)?;
            let table = render(&issues);
            atomic_write(&run.join("ISSUES.md"), table.as_bytes())?;
            print!("{table}");
        }
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn read_issue(path: &Path) -> io::Result<Issue> {
    serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| invalid(format!("{}: {e}", path.display())))
}

fn read_ledger(directory: &Path) -> io::Result<Vec<Issue>> {
    let path = directory.join("issues.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error),
    };
    let issues: Vec<Issue> =
        serde_json::from_slice(&bytes).map_err(|e| invalid(format!("{}: {e}", path.display())))?;
    let mut ids = HashSet::new();
    for issue in &issues {
        if issue.id.is_empty() || !ids.insert(&issue.id) {
            return Err(invalid("ledger contains an empty or duplicate issue id"));
        }
    }
    Ok(issues)
}

/// File an issue and return its stable id. An empty id requests allocation.
pub fn file_issue(directory: &Path, mut issue: Issue) -> io::Result<String> {
    let _lock = LedgerLock::acquire(directory)?;
    let mut issues = read_ledger(directory)?;
    if issue.id.is_empty() {
        let mut number = 1u64;
        while issues.iter().any(|i| i.id == format!("issue-{number}")) {
            number += 1;
        }
        issue.id = format!("issue-{number}");
    }
    if issues.iter().any(|i| i.id == issue.id) {
        return Err(invalid(format!(
            "issue {} already exists; use update",
            issue.id
        )));
    }
    validate(directory, &issue)?;
    let id = issue.id.clone();
    issues.push(issue);
    save(directory, &issues)?;
    Ok(id)
}

fn update_issue(directory: &Path, issue: Issue) -> io::Result<()> {
    let _lock = LedgerLock::acquire(directory)?;
    let mut issues = read_ledger(directory)?;
    let index = issues
        .iter()
        .position(|i| i.id == issue.id)
        .ok_or_else(|| invalid(format!("unknown issue {}", issue.id)))?;
    validate(directory, &issue)?;
    issues[index] = issue;
    save(directory, &issues)
}

fn validate(directory: &Path, issue: &Issue) -> io::Result<()> {
    if issue.id.is_empty()
        || !issue
            .id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
    {
        return Err(invalid(
            "issue id must contain only letters, digits, hyphens, or underscores",
        ));
    }
    if issue.description.trim().is_empty() {
        return Err(invalid("issue description must not be empty"));
    }
    let root = fs::canonicalize(directory)?;
    for evidence in &issue.evidence {
        if evidence.path.as_os_str().is_empty()
            || !evidence
                .path
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(invalid(
                "evidence paths must be relative, without . or .. components",
            ));
        }
        let path = fs::canonicalize(directory.join(&evidence.path))?;
        if !path.starts_with(&root) {
            return Err(invalid("evidence must be inside the ledger directory"));
        }
        let lines = fs::read_to_string(&path)?.lines().count();
        if evidence.line_start == 0
            || evidence.line_end < evidence.line_start
            || evidence.line_end as usize > lines
        {
            return Err(invalid(format!(
                "invalid evidence line range in {}",
                evidence.path.display()
            )));
        }
    }
    Ok(())
}

fn save(directory: &Path, issues: &[Issue]) -> io::Result<()> {
    let mut json = serde_json::to_vec_pretty(issues).map_err(invalid_json)?;
    json.push(b'\n');
    // JSON commits first. If rendering fails, `issues list` rebuilds the view.
    atomic_write(&directory.join("issues.json"), &json)?;
    atomic_write(&directory.join("ISSUES.md"), render(issues).as_bytes()).map_err(|error| {
        io::Error::new(error.kind(), format!("issues.json saved, but ISSUES.md could not be refreshed: {error}; run issues list to retry"))
    })
}

fn invalid_json(error: serde_json::Error) -> io::Error {
    invalid(error.to_string())
}

/// Serialize ledger writers; refuse concurrent updates rather than losing work.
struct LedgerLock(PathBuf);

impl LedgerLock {
    fn acquire(directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let path = directory.join(".issues.lock");
        OpenOptions::new().write(true).create_new(true).open(&path)
            .map_err(|e| io::Error::new(e.kind(), format!("cannot lock ledger ({}): {e}; if a previous writer crashed, remove this lock after confirming no writer is running", path.display())))?;
        Ok(Self(path))
    }
}

impl Drop for LedgerLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn cell(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "&#124;")
        .replace('`', "&#96;")
        .replace('*', "&#42;")
        .replace('_', "&#95;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
        .replace('\\', "&#92;")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "<br>")
}

fn link_path(path: &Path) -> String {
    path.to_string_lossy()
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"/.-_~".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

fn render(issues: &[Issue]) -> String {
    let mut markdown = String::from(
        "# Issues\n\nGenerated from `issues.json`; update issues through fourgate.\n\n| ID | Description | Status | Evidence |\n| --- | --- | --- | --- |\n",
    );
    for issue in issues {
        let status = match &issue.status {
            IssueStatus::NotStarted => "NotStarted".to_owned(),
            IssueStatus::InProgress => "InProgress".to_owned(),
            IssueStatus::NeedsHuman => "NeedsHuman".to_owned(),
            IssueStatus::Resolved(note) => match note {
                Some(note) => format!("Resolved: {note}"),
                None => "Resolved".to_owned(),
            },
        };
        let evidence = issue
            .evidence
            .iter()
            .map(|e| {
                format!(
                    "[{}:{}–{}]({}#L{}-L{})",
                    cell(&e.path.to_string_lossy()),
                    e.line_start,
                    e.line_end,
                    link_path(&e.path),
                    e.line_start,
                    e.line_end,
                )
            })
            .collect::<Vec<_>>()
            .join("<br>");
        markdown.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            cell(&issue.id),
            cell(&issue.description),
            cell(&status),
            evidence
        ));
    }
    markdown
}
