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
