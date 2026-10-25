# fourgate

Use one agent. Start with stages/establish-invocation.md when given a target repo.
Ask for a target path if none is supplied. Claude drives stage 0; the fourgate
CLI only captures commands. It does not launch agents.

Write short. Separate observations verified by running commands from inferences.
Write remotely only to ninehusky/*; never push or comment upstream. Print and
confirm the target before git pushes or GitHub writes. Never merge PRs.

Do not modify vendor/flux. Its gitlink pins the verifier; do not silently update
it or substitute an installed Flux. Preserve user edits and avoid recording secrets.

Use Claude Code's native JSONL transcript for agent history. No custom task event
log or periodic summary is required. Keep exact Flux invocations and full compiler
stdout/stderr separately with `fourgate record`; transcript tool output can be
truncated. See stages/establish-invocation.md for evidence and transcript storage.

ICEs may occur at any stage; never count diagnostics from a rustc panic as complete
results. Trust decisions are explicit and revisitable; stage 0 does not establish
trust or coverage.
