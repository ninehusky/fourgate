//! Source consistency checks, deliberately independent of proof/body coverage.
use crate::{
    diagnostics::{self, ErrorKind, ErrorReport, FluxError},
    flux_issue::{AnchorRole, FluxErrorRef, SourceAnchor, TrustReason},
    issues::{Issue, IssueStatus},
};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Component, Path, PathBuf},
    process::Command,
};
use syn::{Meta, Token, parse::Parser, punctuated::Punctuated};

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}
fn token(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
pub(crate) fn validate_anchor(anchor: &SourceAnchor) -> io::Result<()> {
    if !relative(&anchor.file) || anchor.line == 0 || !token(&anchor.marker) {
        return Err(invalid(
            "anchor requires a relative file, positive line, and lowercase hyphenated marker",
        ));
    }
    if let AnchorRole::Exemption { reason } = &anchor.role {
        let (TrustReason::RealBug(detail) | TrustReason::FluxCrash(detail)) = reason;
        if detail.trim().is_empty() {
            return Err(invalid(
                "retained trust reason must explain the bug or crash",
            ));
        }
    }
    Ok(())
}

/// Reparse retained raw evidence rather than accepting a rewritten errors.json.
pub(crate) fn read_observation(run: &Path, reference: &FluxErrorRef) -> io::Result<FluxError> {
    if !relative(&reference.report)
        || reference
            .report
            .file_name()
            .is_none_or(|name| name != "errors.json")
    {
        return Err(invalid(
            "observation must reference a relative errors.json beneath the ledger",
        ));
    }
    let root = fs::canonicalize(run)?;
    let path = fs::canonicalize(run.join(&reference.report))?;
    if !path.starts_with(&root) {
        return Err(invalid("observation escapes the ledger"));
    }
    let saved: ErrorReport =
        serde_json::from_slice(&fs::read(&path)?).map_err(|e| invalid(e.to_string()))?;
    let actual = diagnostics::read_report(path.parent().unwrap())?;
    if saved != actual {
        return Err(invalid(format!(
            "{} differs from its raw recording",
            reference.report.display()
        )));
    }
    actual
        .errors
        .into_iter()
        .nth(reference.error_index)
        .ok_or_else(|| {
            invalid(format!(
                "error index {} out of range in {}",
                reference.error_index,
                reference.report.display()
            ))
        })
}

struct Source {
    file: PathBuf,
    // Literal contents and block comments are masked; real line comments remain.
    lines: Vec<String>,
    attributes: Vec<(usize, usize)>,
    trusts: Vec<Trust>,
}
struct Trust {
    line: usize,
    source_reason: Option<String>,
}
struct Marker {
    source: usize,
    line: usize,
}

/// Parse attributes even inside macro token trees; this is a lexical inventory,
/// not macro expansion or cfg evaluation. All literal attribute branches count.
fn attributes(tokens: TokenStream, source: &mut Source, masked: &mut [u8]) -> io::Result<()> {
    let tokens: Vec<_> = tokens.into_iter().collect();
    for (i, item) in tokens.iter().enumerate() {
        match item {
            TokenTree::Literal(literal) => {
                for b in &mut masked[literal.span().byte_range()] {
                    if *b != b'\n' {
                        *b = b' ';
                    }
                }
            }
            TokenTree::Group(group) => {
                let outer =
                    i > 0 && matches!(&tokens[i-1], TokenTree::Punct(p) if p.as_char() == '#');
                let inner = i > 1
                    && matches!(&tokens[i-1], TokenTree::Punct(p) if p.as_char() == '!')
                    && matches!(&tokens[i-2], TokenTree::Punct(p) if p.as_char() == '#');
                if group.delimiter() == Delimiter::Bracket && (outer || inner) {
                    let start = tokens[i - if inner { 2 } else { 1 }].span().start().line;
                    source.attributes.push((start, group.span().end().line));
                    match syn::parse2::<Meta>(group.stream()) {
                        Ok(meta) => inspect_meta(&meta, start, &mut source.trusts)?,
                        Err(error) if mentions_exemption(group.stream()) => {
                            return Err(invalid(format!("unparsed exemption attribute: {error}")));
                        }
                        // Macro matchers/interpolated metadata are not ordinary Meta.
                        // Dynamic expansion is outside this lexical gate's scope.
                        Err(_) => {}
                    }
                }
                attributes(group.stream(), source, masked)?;
            }
            _ => {}
        }
    }
    Ok(())
}
fn mentions_exemption(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Ident(ident) => {
            let name = ident.to_string();
            name.contains("trusted") || matches!(name.as_str(), "ignore" | "proven_externally")
        }
        TokenTree::Group(group) => mentions_exemption(group.stream()),
        _ => false,
    })
}
fn inspect_meta(meta: &Meta, line: usize, trusts: &mut Vec<Trust>) -> io::Result<()> {
    let name = meta.path().segments.last().unwrap().ident.to_string();
    let flux_path = meta
        .path()
        .segments
        .iter()
        .any(|segment| matches!(segment.ident.to_string().as_str(), "flux" | "flux_rs"));
    // The pinned Flux parser uses `no` to explicitly turn trust/ignore off.
    if matches!(
        name.as_str(),
        "trusted" | "trusted_impl" | "trusted_derive" | "ignore"
    ) && let Meta::List(list) = meta
        && let Ok(entries) =
            Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())
        && matches!(entries.first(), Some(Meta::Path(path)) if path.is_ident("no"))
    {
        return Ok(());
    }
    if (name == "ignore" && flux_path) || name == "proven_externally" {
        return Err(invalid(format!(
            "unsupported checking exemption {name}; remove it or account for it before using this gate"
        )));
    }
    if matches!(name.as_str(), "trusted" | "trusted_impl" | "trusted_derive") {
        let mut reason = None;
        if let Meta::List(list) = meta {
            let entries = Punctuated::<Meta, Token![,]>::parse_terminated
                .parse2(list.tokens.clone())
                .map_err(|e| invalid(format!("cannot inspect trusted arguments: {e}")))?;
            for entry in entries {
                if let Meta::NameValue(value) = entry
                    && value.path.is_ident("reason")
                {
                    if let syn::Expr::Lit(expr) = value.value
                        && let syn::Lit::Str(literal) = expr.lit
                    {
                        reason = Some(literal.value());
                        continue;
                    }
                    return Err(invalid("trusted reason must be a literal string"));
                }
            }
        }
        trusts.push(Trust {
            line,
            source_reason: reason,
        });
    } else if name == "cfg_attr" {
        if let Meta::List(list) = meta {
            let entries =
                match Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone()) {
                    Ok(entries) => entries,
                    Err(error) if mentions_exemption(list.tokens.clone()) => {
                        return Err(invalid(format!(
                            "cannot inspect cfg_attr exemption: {error}"
                        )));
                    }
                    Err(_) => return Ok(()),
                };
            for entry in entries.iter().skip(1) {
                inspect_meta(entry, line, trusts)?;
            }
        }
    } else if name.contains("trusted") {
        return Err(invalid(format!("unsupported trust attribute {name}")));
    }
    Ok(())
}
fn load_source(file: PathBuf, contents: String) -> io::Result<Source> {
    let tokens: TokenStream = contents
        .parse()
        .map_err(|e| invalid(format!("{}: {e}", file.display())))?;
    // Reject malformed Rust instead of silently overlooking attributes.
    syn::parse_file(&contents).map_err(|e| invalid(format!("{}: {e}", file.display())))?;
    let mut source = Source {
        file,
        lines: vec![],
        attributes: vec![],
        trusts: vec![],
    };
    let mut masked = contents.into_bytes();
    attributes(tokens, &mut source, &mut masked)?;
    // Remove nested block comments after literals have been masked.
    let mut depth = 0usize;
    let mut i = 0;
    while i < masked.len() {
        if depth == 0 && masked[i..].starts_with(b"//") {
            while i < masked.len() && masked[i] != b'\n' {
                i += 1;
            }
        } else if masked[i..].starts_with(b"/*") {
            depth += 1;
            masked[i] = b' ';
            masked[i + 1] = b' ';
            i += 2;
        } else if depth > 0 && masked[i..].starts_with(b"*/") {
            depth -= 1;
            masked[i] = b' ';
            masked[i + 1] = b' ';
            i += 2;
        } else {
            if depth > 0 && masked[i] != b'\n' {
                masked[i] = b' ';
            }
            i += 1;
        }
    }
    source.lines = String::from_utf8(masked)
        .map_err(|e| invalid(e.to_string()))?
        .lines()
        .map(str::to_owned)
        .collect();
    Ok(source)
}
fn source_files(repo: &Path) -> io::Result<Vec<Source>> {
    let root = fs::canonicalize(repo)?;
    let toplevel = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !toplevel.status.success() {
        return Err(invalid("gate requires a Git checkout root"));
    }
    let toplevel = String::from_utf8(toplevel.stdout).map_err(|e| invalid(e.to_string()))?;
    if fs::canonicalize(toplevel.trim_end())? != root {
        return Err(invalid(
            "--repo must be the Git checkout root; subdirectories would omit source",
        ));
    }
    let output = Command::new("git")
        .args(["-C"])
        .arg(&root)
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .output()?;
    if !output.status.success() {
        return Err(invalid(
            "gate requires a Git checkout for source enumeration",
        ));
    }
    let mut files = HashSet::new();
    let mut sources = vec![];
    for file in output.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let file = PathBuf::from(std::str::from_utf8(file).map_err(|e| invalid(e.to_string()))?);
        if !files.insert(file.clone()) {
            continue;
        }
        // The Git index still lists unstaged deletions. The working tree is the
        // gate's source of truth; missing anchors are diagnosed below as usual.
        match fs::symlink_metadata(root.join(&file)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(invalid(format!("{}: {error}", file.display()))),
            Ok(metadata) if metadata.is_dir() => {
                eprintln!(
                    "not scanned: nested repository/submodule {} (outside source-gate scope)",
                    file.display()
                );
                continue;
            }
            Ok(_) => {}
        }
        // Parse configuration structurally so comments, explicit false settings,
        // and unrelated package metadata do not become fragile gate failures.
        if file
            .file_name()
            .is_some_and(|name| name == "Cargo.toml" || name == "flux.toml")
        {
            let path = fs::canonicalize(root.join(&file))?;
            if !path.starts_with(&root) {
                return Err(invalid("configuration symlink escapes target checkout"));
            }
            let content = fs::read_to_string(path)?;
            let document: toml::Value = toml::from_str(&content)
                .map_err(|e| invalid(format!("{}: {e}", file.display())))?;
            let settings = if file.file_name().is_some_and(|name| name == "flux.toml") {
                Some(&document)
            } else {
                document
                    .get("package")
                    .and_then(|v| v.get("metadata"))
                    .and_then(|v| v.get("flux"))
            };
            if let Some(settings) = settings.and_then(toml::Value::as_table) {
                for (key, value) in settings {
                    let normalized = key.replace('-', "_");
                    let bypass = match normalized.as_str() {
                        "default_trusted" | "default_ignore" => value.as_bool() != Some(false),
                        "include_trusted" | "include_trusted_impl" => {
                            value.as_array().is_none_or(|items| !items.is_empty())
                        }
                        _ => false,
                    };
                    if bypass {
                        return Err(invalid(format!(
                            "{}: trust selection configuration {key} is not supported by this gate",
                            file.display()
                        )));
                    }
                }
            }
        }
        if file.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let path = fs::canonicalize(root.join(&file))?;
        if !path.starts_with(&root) {
            return Err(invalid("source symlink escapes target checkout"));
        }
        sources.push(load_source(file, fs::read_to_string(path)?)?);
    }
    Ok(sources)
}

pub(crate) fn check(run: &Path, repo: &Path, issues: &[Issue]) -> io::Result<()> {
    for issue in issues {
        crate::issues::validate(run, issue)?;
    }
    let sources = source_files(repo)?;
    let mut markers = HashMap::new();
    let mut links = HashMap::new();
    for (index, source) in sources.iter().enumerate() {
        for (line, text) in source.lines.iter().enumerate() {
            if let Some(marker) = text.trim().strip_prefix("// fourgate:anchor ") {
                if !token(marker) {
                    return Err(invalid(format!("invalid source marker {marker:?}")));
                }
                if markers
                    .insert(
                        marker.to_owned(),
                        Marker {
                            source: index,
                            line: line + 1,
                        },
                    )
                    .is_some()
                {
                    return Err(invalid(format!("duplicate source marker {marker}")));
                }
            }
            if let Some(id) = text.trim().strip_prefix("// fourgate:trust ") {
                links.insert((index, line + 1), id.to_owned());
            }
        }
    }
    let mut exemptions = HashMap::new();
    for issue in issues {
        let open = !matches!(issue.status, IssueStatus::Resolved(_));
        let Some(tracking) = &issue.flux else {
            if open {
                return Err(invalid(format!(
                    "{}: open issue has no source anchors",
                    issue.id
                )));
            }
            continue;
        };
        if open && tracking.places.is_empty() {
            return Err(invalid(format!(
                "{}: open issue has no source anchors",
                issue.id
            )));
        }
        let mut seen = HashSet::new();
        for anchor in &tracking.places {
            if !seen.insert(&anchor.marker) {
                return Err(invalid(format!(
                    "{}: repeated anchor {}",
                    issue.id, anchor.marker
                )));
            }
            // Resolved obligations can retain historical anchors after code deletion.
            if !open && matches!(anchor.role, AnchorRole::Obligation) {
                continue;
            }
            if !open {
                return Err(invalid(format!(
                    "{}: resolved issue still claims an exemption",
                    issue.id
                )));
            }
            let marker = markers.get(&anchor.marker).ok_or_else(|| {
                invalid(format!("{}: missing marker {}", issue.id, anchor.marker))
            })?;
            let source = &sources[marker.source];
            // At least one code token must follow before the next anchor/EOF.
            let code = source
                .lines
                .iter()
                .skip(marker.line)
                .take_while(|line| !line.trim().starts_with("// fourgate:anchor "))
                .any(|line| !line.trim().is_empty() && !line.trim().starts_with("//"));
            if !code {
                return Err(invalid(format!(
                    "{}: marker {} has no code",
                    issue.id, anchor.marker
                )));
            }
            if source.file != anchor.file || marker.line != anchor.line as usize {
                eprintln!(
                    "{}: {} now at {}:{} (refresh navigation hints)",
                    issue.id,
                    anchor.marker,
                    source.file.display(),
                    marker.line
                );
            }
            if let AnchorRole::Exemption { reason } = &anchor.role {
                let key = (marker.source, marker.line);
                if exemptions
                    .insert(key, (issue.id.as_str(), reason))
                    .is_some()
                {
                    return Err(invalid(
                        "one retained trust cannot be claimed by multiple issues",
                    ));
                }
                let observations = tracking
                    .observations
                    .iter()
                    .map(|r| read_observation(run, r))
                    .collect::<io::Result<Vec<_>>>()?;
                if let TrustReason::FluxCrash(_) = reason
                    && !observations.iter().any(|e| {
                        matches!(e.kind, ErrorKind::Panic | ErrorKind::InternalCompilerError)
                    })
                {
                    return Err(invalid(format!(
                        "{}: flux_crash reason lacks a recorded crash occurrence",
                        issue.id
                    )));
                }
                if issue.evidence.is_empty() && observations.is_empty() {
                    return Err(invalid(format!(
                        "{}: retained trust lacks evidence",
                        issue.id
                    )));
                }
            }
        }
    }
    let mut used = HashSet::new();
    let mut used_links = HashSet::new();
    for (index, source) in sources.iter().enumerate() {
        for trust in &source.trusts {
            // Walk only the contiguous attribute/comment prefix, never past code.
            let mut start = trust.line;
            while start > 1 {
                let previous = start - 1;
                let text = source.lines[previous - 1].trim();
                if text.is_empty()
                    || text.starts_with("//")
                    || source
                        .attributes
                        .iter()
                        .any(|(a, b)| previous >= *a && previous <= *b)
                {
                    start -= 1;
                } else {
                    break;
                }
            }
            let candidates = markers
                .values()
                .filter(|marker| {
                    marker.source == index && marker.line >= start && marker.line <= trust.line
                })
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                return Err(invalid(format!(
                    "{}:{}: trust needs one adjacent source anchor",
                    source.file.display(),
                    trust.line
                )));
            }
            let marker = candidates[0];
            let key = (index, marker.line);
            let (id, reason) = exemptions.get(&key).ok_or_else(|| {
                invalid(format!(
                    "{}:{}: temporary or unaccounted trust; remove it before committing",
                    source.file.display(),
                    trust.line
                ))
            })?;
            let backlinks = links
                .iter()
                .filter(|((s, l), _)| *s == index && *l > marker.line && *l < trust.line)
                .collect::<Vec<_>>();
            if backlinks.len() != 1 || backlinks[0].1.as_str() != *id {
                return Err(invalid(format!(
                    "{id}: trust backlink missing or disagrees with issue"
                )));
            }
            used_links.insert(*backlinks[0].0);
            // If the checker accepts a reason argument, keep its category in sync.
            let category = match reason {
                TrustReason::RealBug(_) => "real_bug",
                TrustReason::FluxCrash(_) => "panic",
            };
            if trust
                .source_reason
                .as_ref()
                .is_some_and(|value| value != category)
            {
                return Err(invalid(format!(
                    "{id}: source trust reason disagrees with {category}"
                )));
            }
            if !used.insert(key) {
                return Err(invalid(format!(
                    "{id}: one anchor accounts for multiple trust attributes"
                )));
            }
        }
    }
    if used.len() != exemptions.len() {
        return Err(invalid(
            "issue claims an exemption without a matching trust attribute",
        ));
    }
    if used_links.len() != links.len() {
        return Err(invalid("orphan or duplicate trust backlink"));
    }
    Ok(())
}
