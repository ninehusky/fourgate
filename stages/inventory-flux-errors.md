# Inventory observed Flux errors

Goal: collect observed errors while temporarily unblocking dependencies. This is
not a complete error inventory or a proof/body-coverage gate. CLI commands record
and link evidence; the agent performs investigation and target edits.

1. Recover one exact target checkout, toolchain, Cargo target/features/platform,
   and repeatable Flux invocation. Use the pinned verifier and preserve user edits.
   Keep all attempts beneath one ledger directory, e.g. `runs/inventory/attempt-01`.
2. Inspect `cargo metadata` for that configuration. With `p -> q` meaning p depends
   on q, investigate q first. Distinguish normal/build/dev edges and target kinds;
   do not assume a package-level success means all binaries, tests, or dev targets
   were checked. Preserve narrowed invocations in the recordings.
3. Record each attempt with `fourgate flux`. Inspect both streams for `panicked`
   and ICEs before reporting results. Never quote error totals from a rustc-panic
   run. Ordinary errors, dependency blocks, and compiler crashes are different
   observations; a crash-free run does not establish that dependents ran.
4. For each distinguishable observed obligation, add a source marker and use
   `issues observe` to create or update its issue. Group only with explicit
   evidence; do not deduplicate by spans or error-message equality. Use
   `issues file/update` for blockers that are not represented by a diagnostic.
5. Temporarily trust the smallest blocking bodies, then recheck the same target.
   Save each exemption's issue backlink and exact patch with the attempt. These
   discovery exemptions are experimental, not committed DeferredProof trusts.
   If trust does not bypass the failure, record the blocker instead of broadening
   permanent trust. Use the existing crash-isolation workflow for ICEs.
6. Repeat while new errors appear, then move to dependent targets with the same
   configuration and explicit assumption patch. Record which targets remain
   blocked and which were actually checked. Do not infer resolution from absence:
   earlier errors and dependencies may mask diagnostics.
7. Retain immutable recordings, the inventory, and the temporary exemption patch.
   Remove discovery trusts from target code before committing. Keep obligation
   markers so open issues still map to current code. Only validated RealBug or
   FluxCrash exemptions may remain, linked to open issues and supporting evidence.
8. Run `issues gate --run LEDGER --repo TARGET` after restoring the intended target
   patch. This checks source bookkeeping, not complete checking. Also run
   `gate-flux --input ATTEMPT` when asserting a recording is interruption-free;
   it can pass while ordinary errors/dependency masking remain.

To work an issue later: remove its retained exemption (if any), reproduce without
it, repair the code, and record actual checking. Update/move anchors with the
obligation. Close it only with evidence of repair or explicit code removal; a
blocked recheck leaves it open. An experimental positive run using trusted
prerequisites is conditional on those prerequisites and must retain that patch.

Binary-site matching, call graphs, and behavioral body/closure coverage probes
are separate follow-ups. FluxErrorRef preserves report-local endpoints for those
future relations; it does not assert a source or machine-code match.
