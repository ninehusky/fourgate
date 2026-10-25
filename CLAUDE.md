# fourgate

Use one agent. Start with stages/establish-invocation.md when given a target repo.
Ask for a target path if none is supplied. This repo currently implements stage 0
only, not coverage or proof automation.

Write short. Separate observations verified by running commands from inferences.
Write remotely only to ninehusky/*; never push or comment upstream. Print and
confirm the target before git pushes or GitHub writes. Never merge PRs.

Do not modify vendor/flux. Its gitlink pins the verifier; do not silently update
it or substitute an installed Flux. Preserve user edits and avoid recording secrets.

Keep each task's exact commands and raw logs. Record concise evidence-backed
findings, blockers, and next actions in summary.md. ICEs may occur at any stage;
never count diagnostics from a rustc panic as complete results. Trust decisions
are explicit and revisitable; stage 0 does not establish trust or coverage.
