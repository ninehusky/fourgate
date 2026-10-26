# fourgate

Use one agent. Start with stages/get-flux-running.md when given a target repo.
Ask for a target path if none is supplied. Claude drives the pipeline; the
fourgate CLI captures commands and runs deterministic binary analysis. It does
not launch agents. See stages/measure-panics.md for the measurement stage.

Pipeline:
1. Get Flux running — set up the checker on the codebase and save a repeatable command.
2. Check coverage — confirm every body/closure is checked or explicitly exempted.
3. Prove panic freedom — work through the proof obligations.

Only the first proof step is implemented here; binary measurement is separate.
Flux repair can be needed throughout;
these steps are not a one-time pass through the codebase.

Write short. Separate observations verified by running commands from inferences.
Write remotely only to ninehusky/*; never push or comment upstream. Print and
confirm the target before git pushes or GitHub writes. Never merge PRs.

Do not modify vendor/flux. Its gitlink pins the verifier; do not silently update
it or substitute an installed Flux. Preserve user edits and avoid recording secrets.

Use Claude Code's native JSONL transcript for agent history. No custom task event
log or periodic summary is required. Keep exact Flux invocations and full compiler
stdout/stderr separately with `fourgate record`; transcript tool output can be
truncated. See stages/get-flux-running.md for evidence and transcript storage.

ICEs may occur at any stage; never count diagnostics from a rustc panic as complete
results. Trust decisions are explicit and revisitable; getting Flux running does
not establish trust or coverage.
