# fourgate

just four gate every game and you will win at least 50% of your protoss vs. protosses

`fourgate record` preserves build/checker commands and logs.

`fourgate flux --repo /path/to/repo --output /path/to/new-run -- --workspace`
runs `cargo flux --message-format=json` at the supplied repo root, records the
invocation, stdout, stderr, and result, and returns the command's exit code.
It also writes `errors.json`: errors with source spans and raw-log references,
explicit ICEs distinguished from plain panics, and incomplete-run reasons.
Warnings stay in the raw logs. See the [log/report examples](tests/fixtures/flux/README.md).
Extraction failures panic; raw logs and the command's result remain available.
It requires `cargo-flux` installed from the pinned `vendor/flux` revision.

`fourgate parse-flux --input /path/to/recorded-run` parses saved logs into the
same schema on stdout without running Flux or changing the recording. Missing
`result.json` produces an incomplete report. Exit zero means extraction succeeded,
even if the recorded run crashed; inspect `complete` and `incomplete_reasons`.

`fourgate gate-flux --input /path/to/recorded-run` exits zero only for a complete
diagnostic inventory. ICEs, panics, interrupted runs, launch failures, and unreadable
or malformed evidence fail the gate. It reparses the raw recording rather than
trusting `errors.json`. Ordinary verification errors may pass this gate; it does
not establish proof coverage or verification success. Run it after each repair
and on the final broad run, even when `fourgate flux` returned a nonzero exit.
Cargo-reported compiler subprocess signals also fail, even when Cargo itself
returns a normal error exit alongside other diagnostics.

For the crash-repair loop in Claude Code, invoke `/address-ices runs/NAME`.
The [repair skill](.claude/skills/address-ices/SKILL.md) repeatedly isolates crashes,
tests narrow trust exemptions, and updates `ISSUES.md`. It continues until the
broad interruption gate passes or remaining work needs human judgment. Every
retained exemption leaves an open follow-up for removing it; resolving the crash
does not discard that work. Supply the target and checker setup if they are not already recorded.

`fourgate issues file|update|list --run runs/NAME` manages the run's typed todo
ledger and generates its `ISSUES.md` table. See [ledger commands and JSON input](stages/issues.md).
The schema lives in `src/issues.rs`; parser errors stay independent of these todos.
Keep repair attempts, patches, the repeatable command, native agent transcript,
and available token/time measurements together in `runs/NAME/` (ignored by Git).

`fourgate measure-panics --binary ELF --output NEW_DIRECTORY`
inventories recognized panic sites in machine code. See [Measure panics](stages/measure-panics.md).
