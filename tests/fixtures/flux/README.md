# Error-report examples

Report examples pair recorder files or excerpts with a hand-authored
`expected.json`; `malformed_json` instead expects a parser panic.
`fourgate flux` generates the report automatically after
recording its command. The schema is in
[`src/diagnostics.rs`](../../../src/diagnostics.rs).

| Example | Expected behavior |
|---|---|
| `success` | An ordinary successful run produces a complete, empty error list. |
| `diagnostic` | Keep a located error; omit a warning. A normal error exit can have a complete report. |
| `crashes` | Associate one JSON ICE with its matching panic block. Keep two identical text-only panics separate, with unknown package/target. |
| `real_crashes` | Real Firecracker run excerpts: two explicit ICEs, each with a panic block, and two plain `Tuple3` panics. |
| `interrupted` | A signal exit produces an incomplete report. |
| `malformed_json` | Truncated diagnostic JSON makes the parser panic; no report is produced. |
| `launch_failure` | Report a command launch failure, citing `result.json`. |

`complete` means the command finished without crashing and error extraction
succeeded. It does not mean every workspace package was checked. Missing
`result.json` or an absent exit code without a launch error indicates an
interrupted recording. A recorded launch error is an invocation failure.

Error kinds distinguish `internal_compiler_error` (an explicit ICE diagnostic)
from `panic` (a panic without an associated ICE diagnostic). An ICE can also
print a panic block: that is one occurrence with both pieces of evidence.
Both kinds make the report incomplete with reason `compiler_crash`.

`real_crashes` contains verbatim excerpts from
`runs/firecracker-flux-20261029/attempt-02`: stdout lines 177 and 196; stderr
lines 178–179, 353–354, 452–453, and 585–586. Blank lines separate the stderr
excerpts. Evidence coordinates in its report refer to the fixture files.

Evidence uses `file` rather than `log`, since launch failures live in
`result.json`. Evidence line ranges are inclusive; source span ends are
exclusive, matching rustc. Source paths are relative to the recorded invocation
when supplied that way. Raw file contents remain unchanged.

For concurrent compiler output, nearby verbose lines do not establish which
function or package caused a text-only panic. The `crashes` example includes a
misleading neighboring function deliberately.

The parser reads Cargo/rustc JSON and plain-text errors and panic blocks from
both output streams. Malformed JSON, invalid UTF-8, invalid spans, or a nonzero
exit without any recovered error make the parser panic. Extraction failure is
not a report status. Raw evidence and the command's result remain intact, and
no `errors.json` is written. Unreadable recording files are returned as I/O errors.
Missing result metadata or a signal exit remains a reportable interruption
when the logs themselves are well-formed.

Only one ICE and one panic at the same compiler file/line/column are associated.
Ambiguous matches remain separate. Backtrace evidence extends through contiguous
frames; interleaved output can truncate that range. The full raw logs are always
preserved. The parser does not infer source locations from backtraces or nearby
verbose function names.

`cargo test --locked --test error_report` compares parsed output with every
expected report and checks schema round-trips and evidence bounds. CLI tests in
`tests/flux.rs` verify automatic reports, argument forwarding, raw-log preservation,
and exit codes using a fake Cargo executable.
