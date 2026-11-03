---
name: repair-flux-interruptions
description: Bisect reproducible Flux compiler crashes and verify narrowly scoped trusted annotations in a target checkout. Use for ICEs or compiler panics, not ordinary refinement errors or checker setup failures.
---

Repair a catastrophic Flux interruption by isolating the responsible body and
exempting it with a verified `trusted` annotation. This restores checker progress;
it does not prove the exempted code safe. Use one agent. Write short.

## Establish the baseline

- Read the target's instructions. Work in an isolated target checkout, preserving
  user edits. Use fourgate's pinned `vendor/flux` revision; do not modify it.
  If setup is missing, follow [Get Flux running](../../../stages/get-flux-running.md).
- Identify the target path, exact command, configuration, toolchain, and existing
  trust annotations from the recorded invocation and source. Ask for the target
  only if it cannot be recovered. Keep prerequisites and patches with the evidence.
- Use the absolute built fourgate binary to record each attempt in a new directory
  outside the target: `fourgate flux --repo TARGET --output NEW_RUN -- ARGS`.
  Parse older recordings with `fourgate parse-flux --input RUN`. Neither command's
  exit code alone establishes a complete run; inspect the report and raw logs.
- Inspect both streams for `panicked` and explicit ICEs before quoting results.
  `compiler_crash` warrants this workflow. A signal, timeout, missing result, or
  `invocation_failure` alone does not identify a body to trust. Fix prerequisites
  or reproduce the compiler crash first. Malformed logs are extraction failures,
  not evidence of a successful repair.
- Reproduce the crash on unchanged source. Describe its signature using compiler
  location/message and raw evidence, not a diagnostic count. Treat adjacent
  verbose function names as candidates only, especially with concurrent output.
  Reuse an existing recording if its source, invocation, environment, and checking
  witness match; do not rerun merely to rename it as a baseline.

## Choose the next experiment

Before a checker run, state the question it answers and the previous recording
that does not answer it. Reuse matching evidence. Keep a short index of signature,
source/configuration, tested exemption set, and outcome alongside the attempts;
this is an experiment index, not another agent transcript.

- A diagnostic span inside one body is enough to test that body's trust directly.
  An isolated package's last verbose body is a hypothesis to test directly, not a
  reason to start a whole-file bisection. Nearby concurrent output is insufficient.
- Use bisection when the culprit is unknown or a direct candidate fails to explain
  the signature. Do not expand a singleton back into a larger candidate set.
- Validate the recording helper's argument forwarding without Flux first. Keep
  helper options before a required `--` separator; inspect the recorded argv.
  Argument rejection, no matched bodies, and dependency aborts are invalid
  experiments, not crash-free evidence. Fix that mechanism before another trial.
- Read [pinned Flux pitfalls](references/pinned-flux.md) when selecting bodies or
  exempting generated code. Do not rediscover a known failed mechanism on the
  same verifier revision.

## Bisect the checked bodies

Keep the baseline source and broad invocation recoverable. Record each tested set,
temporary change, command, and observed signature in the native session transcript;
keep the full recordings and source patches separately.

1. Narrow a broad workspace run to the failing package/target, then files and
   bodies. Preserve features, target triple, dependencies, and checker settings.
   Confirm each narrowing still reproduces the same signature.
2. Split the candidate bodies into two groups. Temporarily trust one group and
   rerun. If the same signature persists, the untrusted remainder is the next
   candidate set; if a valid run clears it, narrow the trusted group. Restore
   temporary group changes before the next trial. Use temporary
   body annotations or the pinned Flux's selection flags after checking their
   semantics in `vendor/flux/crates/flux-config/src/{lib,flags}.rs`. Selection
   patterns can match multiple definitions; record what actually matched.
   Prefer trust groups over `include` filters when exclusions
   change dependency metadata. Def patterns omit the crate prefix; verify actual
   checked bodies and solved constraints, since a no-match can exit zero.
3. Keep splitting the group whose checking reproduces the crash. A run qualifies
   as crash-free only if checking actually ran and reached completion without
   another interruption. Ordinary proof errors may remain. Force recompilation
   with a fresh Cargo target directory when cache reuse could hide checking;
   preserve and record the relevant environment, including `FLUXFLAGS`.
4. Test the complementary group when the result changes signature, is inconsistent,
   or may depend on interacting bodies. A single reproducible culprit does not
   require both halves at every level. If both groups crash, track each signature
   and isolate each separately. If
   neither crashes but the union does, test interacting groups and reduce that
   combination; do not invent a single culprit. Repeat an inconsistent result
   before drawing a conclusion. A changed crash is a new observation, not success.
   Preserve the observed SIGKILL or timeout as its own signature; do not label it
   OOM without evidence.

Temporary trust or selection may not bypass a crash in parsing, desugaring, or
signature processing. If body-level trust cannot isolate it, stop with the
smallest reproducer and recordings. Do not widen permanent trust to a module or
crate merely to make the command finish. Also stop if prerequisites prevent
comparable runs or repeated experiments cannot reproduce the signature.

## Verify the permanent exemption

- Remove all experimental annotations and selection restrictions. Add only the
  isolated body's `#[flux::trusted]` (or the equivalent attribute already used by
  this target). Keep any needed conditional compilation convention. Add a short
  source comment stating the crash reason and pointing to durable evidence.
- Require a recorded negative arm without each exemption and a positive arm with
  it under comparable conditions. Discovery runs can supply these arms; do not
  repeat them as a separate ritual. A later crash is evidence of progress past
  the old crash, not a positive arm for the complete repair. Once the retained set
  is stable, test removals only where existing evidence does not establish
  necessity in that set (including interacting or redundant exemptions). Restore
  the set and use the final broad run as the shared positive arm.
- Finally rerun the original broad invocation with only the retained annotations,
  no experimental filters, and evidence that Flux checked the intended code.
  Resolve newly exposed crashes through the same process. A different crash,
  skipped compilation, launch failure, signal, or extraction failure cannot count
  as completion. Ordinary diagnostics may remain; `complete` is not proof coverage.
- Require `fourgate gate-flux --input FINAL_RUN` to pass before declaring repair
  complete. This gate reparses raw evidence and rejects any incomplete report;
  ordinary verification failures can pass. Still confirm the intended code ran.
- For generated bodies that cannot carry source attributes, use the pinned
  verifier's narrow `include-trusted` entries and preserve them as retained trust
  configuration with the final command. Test each entry's necessity. Do not keep
  a `trusted_derive` annotation that failed to bypass the generated crash.
- Keep the final patch and baseline/final recordings. Report which bodies became
  trusted and why, what was verified by running, and what remains inferred or
  blocked. Do not quote diagnostic totals from a rustc panic run. Do not claim
  panic freedom or coverage for exempted bodies.
  Reuse a broad run that already has the exact final source, retained trust,
  original scope, and fresh-checking evidence. Do not run separate "broad",
  "final", and "final-fresh" copies; arrange freshness before that one run.

## Preserve the repair run

Use `runs/NAME/` in fourgate for immutable attempt directories, source patches,
the repeatable final command, and the native agent transcript. Snapshot only this
session's raw JSONL, not unrelated chats. Record start/end timestamps and elapsed
time; save native token usage when available, with a baseline and final snapshot
so earlier conversation usage is distinguishable. Missing usage is unknown, not
zero. Keep any separately consulted agent's transcript and usage separate.
