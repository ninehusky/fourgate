---
name: address-ices
description: Repeatedly isolate and quarantine Flux compiler crashes, tracking exemptions and human blockers in the issue ledger.
disable-model-invocation: true
argument-hint: "[run-directory]"
---

# Input

Run directory: `$ARGUMENTS`

The argument is a ledger directory under `runs/`, containing checker attempts,
patches, and setup notes. Recover the target checkout, pinned Flux revision,
original broad invocation, and required environment from that evidence and the
user's request. Ask only for missing prerequisites. An empty directory needs a
target and a repeatable checker command before investigation can begin.

Read `CLAUDE.md` and the target's instructions. Use one agent. Keep edits in an
isolated target checkout, preserving user work. Do not modify `vendor/flux`,
change verifier revisions, push, or merge.

# Work

Repeat the crash-repair loop until the original broad run passes the interruption
gate, or remaining work requires human judgment. A validated narrow body-level
trust exemption is authorized: record it and continue without asking for approval
after every body. Keep prior retained exemptions in subsequent runs.

1. Run `fourgate flux --repo TARGET --output RUN/ATTEMPT -- ARGS` with the pinned
   toolchain and saved environment. Use a new attempt directory each time.
   Reuse an existing baseline only when its source, invocation, environment, and
   evidence of actual checking match. If Firecracker lives in Docker, run the
   checker there and copy the complete recording into the host ledger directory.
   Record the container path mapping and environment in setup notes. Parse copied
   raw recordings with `fourgate parse-flux` if needed. No custom wrapper is required.
   `fourgate flux` already supplies `--message-format=json`; do not pass it again.
2. Inspect `errors.json` and both raw streams for panics and ICEs before quoting
   results. Never quote diagnostic totals from a compiler-panic run. A signal,
   setup failure, or parser failure alone is not grounds to trust a body.
3. File or resume issues for distinguishable observed crashes, then select one
   actionable crash issue to investigate. Open proof/exemption-removal follow-ups
   are separate work; do not select them merely because they are NotStarted. Describe the
   observed signature separately from hypotheses. A todo can cover related
   diagnostics; do not automatically file an issue for every diagnostic.
4. Reproduce and isolate that signature. Follow the baseline, experiment selection,
   and bisection guidance in
   [the isolation workflow](../../../.agents/skills/repair-flux-interruptions/SKILL.md).
   Read its linked verifier pitfalls when relevant. Test a directly supported
   candidate before bisecting larger sets. Nearby concurrent output is not proof
   of the failing body. Preserve features, target, and dependency configuration.
5. Prepare the smallest body-level trust exemption. Verify the pinned verifier's
   attribute syntax; do not assume `trusted(reason = ...)` is supported. Include
   the crash location/reason in the supported annotation or an adjacent comment.
   Remove experimental trust groups and filters. Keep the original source patch
   recoverable. Do not broaden trust to a whole file or crate to force completion.
6. Compare recorded runs without and with the proposed exemption. Ensure the
   intended code actually ran, invalidating local fingerprints when necessary.
   Reuse equivalent experiments. Rerun the original broad invocation with all
   retained exemptions and no experimental filters, and run `fourgate gate-flux --input ATTEMPT`.
   If another crash remains, say that this signature cleared, not that the entire
   run was repaired. Never treat a dependency abort or cached/no-match run as a
   positive control.

After each validated exemption, update its issue and inspect the latest broad
recording for remaining or newly exposed crashes. Reuse that recording for the
next iteration when it answers the next question. Do not repeat identical trials
without a new hypothesis. Reopen an issue if its signature recurs; do not file a
duplicate just because it appears in another attempt. Signature matching is an
agent judgment backed by evidence, not automatic diagnostic merging.

A crash is quarantined only when comparable controls establish that the retained
exemption clears its signature. A changed crash can hide the old one; isolate
further rather than declaring success without evidence. Record recurring or
unexplained outcomes as blockers instead of cycling indefinitely.

If body-level isolation fails or no supported narrow exemption works, mark the
issue `NeedsHuman`. Describe the smallest reproducer and the decision needed:
exclude/quarantine a wider file, rewrite the offending code, or patch Flux. Do
not silently widen exclusions, rewrite behavior, or change the verifier. Continue
other actionable issues only if comparable checking is still possible; otherwise
stop with the blocker. Do not retry a human-blocked issue without new evidence or
a human decision.

# Progress

Use `fourgate issues file|update|list --run RUN` to maintain `issues.json` and its
generated `ISSUES.md` table. See [the ledger commands](../../../stages/issues.md)
for input JSON and status encoding. Never edit the Markdown table directly.

File with `NotStarted`, then use `InProgress` while investigating.

Before resolving a crash by quarantine, file or reuse an **open follow-up for
each retained body exemption or generated-code trust span**. Use the existing
Issue schema: `NotStarted`, a description naming the exact body/source location,
the annotation or trust flag, the originating crash issue ID, and the work needed
to remove the exemption (fix Flux, rewrite code, then check the body). Copy the
relevant patch/flag and negative/positive evidence references. Name the follow-up
ID in the crash's nonempty resolution note. No new issue type or schema is needed.

Create the follow-up first. If filing it fails, do not resolve the crash. On resume,
reuse an existing open follow-up for that exact exemption rather than duplicating
it. Audit prior resolved crash issues and retained trust configuration too: add
missing follow-ups before declaring this invocation complete. A crash covering
several retained derive spans needs a follow-up for each span.

`Resolved` on the crash means its interruption was bypassed; the follow-up stays
open while the exemption remains, even after a gate pass or human acceptance.
Close the follow-up only when its exemption is removed and a recorded run shows
the intended body is actually checked without that exemption. State any remaining
ordinary proof obligations; do not claim panic freedom without discharging them.

Reserve `NeedsHuman` for a blocker or a remedy outside the authorized body-level
exemption workflow. Update after a material finding or experiment, retaining prior
evidence. Keep the description short: problem, current diagnosis, retained or
proposed body/change, validation, and any specific decision needed. Put lengthy
details in linked evidence files.

Evidence paths are relative to RUN, with one-based inclusive line ranges. Keep
attempts immutable. Save the target patch, exact commands/environment, and this
session's native transcript alongside the recordings. JSON is the ledger; the
transcript remains the detailed history, not a second custom event stream.

# Review

Keep patches and evidence reviewable as you go. For each retained exemption or
human blocker, identify:

- The source file/body and crash signature.
- The proposed trust exemption and the proof obligation it bypasses.
- The negative/positive recordings, broad gate outcome, and remaining uncertainty.
- For a blocker, the exact decision needed and the available remedies.

If blocked, use `NeedsHuman` with the blocker, last useful evidence, and what is
needed to continue. Do not imply successful isolation. Leave any unvalidated
candidate patch separate from the validated retained exemptions.

# Finish

Finish when a fresh original-scope recording passes `gate-flux` with evidence
that intended checking ran, or remaining crashes cannot be advanced within the
authorized workflow. Ordinary verification errors may remain. A compiler crash,
signal, cached/no-match run, or extraction failure is not successful completion.

Before finishing, reconcile the retained patch and trust flags against open
follow-ups: every exempted body/span must have one. An incomplete reconciliation
is a ledger blocker, not a successful handoff.

Link `RUN/ISSUES.md` in a short final reply. Summarize retained exemptions, the
final recording/gate result, and human blockers. Keep the ledger and recordings
for resumption; never reset them merely because the invocation ended. Regenerate
`ISSUES.md` from `issues.json` if needed. A separate experiment may use a new run
directory. Gate success establishes neither proof coverage nor panic freedom.
