# Issue ledger

One directory holds the ledger (`issues.json`), its generated table (`ISSUES.md`),
and immutable checker attempts/patches. Issues are todos, independent of parser
`FluxError` objects. Several related diagnostics can support one issue.

Use `fourgate` below, or `cargo run --locked --` from the repository root.
These commands manage the ledger; they do not launch Claude or run Flux.

## File an issue

Save a complete Issue as JSON, for example in `runs/demo/draft.json`:

```json
{
  "id": "",
  "description": "Compiler panic in baseline; isolate the responsible body.",
  "status": "not_started",
  "evidence": []
}
```

```sh
fourgate issues file --run runs/demo --input runs/demo/draft.json
```

An empty ID allocates `issue-1`, `issue-2`, etc. A supplied ID must be unique.
The command prints the assigned ID and refreshes `ISSUES.md`.

## Update and review

Copy the current issue from `issues.json` into an input file, edit it, then run:

```sh
fourgate issues update --run runs/demo --input runs/demo/update.json
fourgate issues list --run runs/demo
```

Update replaces the complete issue by ID. Preserve prior evidence when adding
new observations. Unknown IDs and duplicate filings fail without changing the
ledger. List prints the table and rebuilds `ISSUES.md`; edit JSON through the
commands, not the generated Markdown.

Statuses are `"not_started"`, `"in_progress"`, `"needs_human"`, or
`{"resolved": null}` / `{"resolved": "Fixed the underlying problem."}`.
For a trust exemption, first file a separate `not_started` follow-up for each
retained body or generated-code span. Name the body/location, annotation or trust
flag, and original crash ID, and copy the supporting evidence. Then resolve the
crash with a nonempty note such as `"Bypassed via trusted on set_before; remains
unproved; removal tracked in issue-9."` Reuse existing follow-ups when resuming.
Do not resolve the crash if filing its follow-up failed.

Follow-ups stay open while their exemptions remain. A gate pass or acceptance of
the quarantine does not close them. Closing one requires removing the exemption
and recording actual checking without it; any remaining ordinary proof work must
still be stated. These are instructions for the agent, not CLI-enforced transitions;
the generic ledger has no crash-specific issue type. `needs_human` means a blocker requiring a
decision, not routine approval after every narrow exemption.

Keep the ledger across invocations. Reopen recurring problems and add new ones;
do not reset it after a gate pass. Use a separate run directory for a separate
experiment. `ISSUES.md` can always be regenerated from the retained JSON.

Use the description for the diagnosis, proposed repair, validation, and the
specific human decision. No automatic diagnostic matching or dependency graph.

Evidence uses existing UTF-8 files beneath the ledger directory:

```json
{"path": "baseline/stderr.log", "line_start": 12, "line_end": 15}
```

Ranges are one-based and inclusive. Empty evidence is allowed. The table links
to the files and labels the ranges; whether the viewer jumps to the line depends
on its support for line fragments. Preserve recordings so old references remain
meaningful after source edits. A patch or setup note can also be evidence.

## Persistence

Commands reject malformed JSON and invalid new evidence, serialize writers with
`.issues.lock`, and replace files through temporary files. `issues.json` commits
before the Markdown view: if refreshing Markdown fails, the error says the JSON
was saved; use `issues list` to retry. Do not retry filing as a new issue.

After a killed writer, inspect `issues.json` before resuming. Remove stale
`.issues.lock` or `*.tmp` files only after confirming no writer is running.
The ledger is run-local and not a database or a transcript replacement.

## Code-linked Flux issues

The optional `flux` field adds source anchors and references to recorded errors.
Older generic todos still load and render; to pass the source consistency gate,
every unresolved issue in this ledger must have at least one source anchor.
One ledger is scoped to one target checkout, supplied to the gate with `--repo`.

```json
{
  "id": "FLUX-017",
  "description": "Prove packet indexing is in bounds.",
  "status": "not_started",
  "evidence": [],
  "flux": {
    "places": [{
      "file": "src/packet.rs",
      "line": 84,
      "marker": "packet-bounds",
      "role": {"kind": "obligation"}
    }],
    "observations": [{"report": "check-042/errors.json", "error_index": 0}]
  }
}
```

Place `// fourgate:anchor packet-bounds` immediately before the relevant code or
its attributes. Tokens match `[a-z0-9]+(-[a-z0-9]+)*` and must be unique throughout
the checkout. `file` and one-based `line` are navigation hints, not identity;
the gate searches by marker and reports moved locations without rewriting JSON.
When moving code, move its marker. Splitting/deleting obligations requires an
explicit issue update; a missing diagnostic never resolves an issue automatically.

To link an observed error, save the SourceAnchor object above as `anchor.json`:

```sh
fourgate issues observe --run runs/demo --report check-042/errors.json \
  --error-index 0 --anchor anchor.json
# Further occurrences of the same obligation:
fourgate issues observe --run runs/demo --report check-043/errors.json \
  --error-index 1 --issue issue-1
```

The first command allocates an issue and uses the diagnostic message as its
initial description. Review that description: the parser's Diagnostic kind does
not establish a real bug or automatically distinguish refinement errors from
ordinary compiler errors. Related occurrences may share an issue; repeated links
to the same report entry are deduplicated. Reopening a resolved issue is explicit.
Reports and their raw logs must remain under the ledger directory and immutable.
The CLI reparses raw evidence and rejects report contents that disagree with it.
No `ArtifactRef` or second issue store is introduced.

## Source/trust consistency gate

```sh
fourgate issues gate --run runs/demo --repo /absolute/target/checkout
```

This can run in CI before committing target changes. It requires:

- Every unresolved issue has a current, unique source marker with code after it.
- Every literal Rust trust attribute has one adjacent anchor, an exact
  `// fourgate:trust ISSUE-ID` backlink, and a matching open exemption entry.
- Every exemption claimed by an issue has a matching trust in source.
- Every observation resolves to the same error in retained raw evidence.

For a retained trust, the anchor's role becomes:

```json
{"kind":"exemption","reason":{"kind":"real_bug","detail":"Confirmed out-of-bounds access; maintainer fix pending."}}
```

The other accepted reason is `flux_crash`, with a nonempty explanation and a
linked recorded ICE/panic occurrence. `real_bug` requires supporting evidence and
human/agent confirmation; a failed proof alone is insufficient. Source placement:

```rust
// fourgate:anchor packet-bounds
// fourgate:trust FLUX-017
#[flux::trusted]
fn decode() { /* ... */ }
```

If an annotation has a literal `reason` argument, its category must be `real_bug`
for RealBug or `panic` for FluxCrash. Explicit `trusted(no)`/`ignore(no)` annotations are not exemptions.
The gate does not require a reason argument:
use the actual syntax supported by the target verifier, with the explanation in
the issue. Resolving an issue requires removing its exemption entry as well as
its annotation/backlink; resolved obligations may retain historical locations.

Temporary trusts used to collect errors are **not** accepted retained exemptions.
Save them as an experiment patch, then remove them before the commit gate. No
DeferredProof reason exists. The gate is opt-in and does not install a Git hook.

Scope: Git-listed tracked and nonignored untracked `.rs` files under the Git
checkout root passed as `--repo` (unstaged deletions are omitted),
including literal `cfg_attr` branches and attributes written inside macro token
trees. It does not expand macros, enumerate ignored/generated Rust, inspect
external dependencies, Git submodules/nested repositories, or establish
compilation/body coverage. Listed nested directories are reported as not scanned.
Known `ignore`/`proven_externally` Flux attributes and
`include-trusted`/`include_trusted`, `default-trusted`/`default_trusted`, and
`default-ignore`/`default_ignore` TOML settings are rejected as
unsupported when enabled/nonempty rather than accepted as accounted exemptions.
The gate parses `flux.toml` and `[package.metadata.flux]` in `Cargo.toml`; comments,
false defaults, empty selectors, and unrelated metadata are accepted. Other
checking flags and build configurations remain part of run provenance, outside
this source gate.
