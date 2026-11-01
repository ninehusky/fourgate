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

`fourgate measure-panics --binary ELF --output NEW_DIRECTORY`
inventories recognized panic sites in machine code. See [Measure panics](stages/measure-panics.md).
