# Pinned Flux pitfalls

Observed during `runs/firecracker-repair-20261102/` on Flux
`1b04d5e6f8247766cfb0f9c6040c2944eb9c87c4`. Recheck implementation if the
gitlink changes; these are not promises about future Flux versions.

- `def:` patterns use rustc definition paths without a crate prefix. A prefixed
  `def:clippy_tracing::apply` matched nothing. Check the intended body's verbose
  witness; dependency summaries alone do not establish that it ran.
- `include` filters can omit dependency definitions needed by the target.
  Selecting only Jailer's constructor instead produced an external-enum error
  before the crash. Prefer temporary `include-trusted` groups, which preserve
  the rest of the checking configuration and metadata.
- `trusted_derive` did not suppress nested serde `visit_enum` bodies on
  `SeccompCmpOp`. A span trust entry at the `Deserialize` token did. For another
  crash in that generated shape, start with a narrow derive-site trust test;
  do not repeat the ineffective root-only annotation. Each distinct site's
  necessity still needs evidence. Retain the exact source revision and spans.
- Body trust is applied after trait-implementation subtyping in
  `crates/flux-refineck/src/lib.rs`. A crash in that earlier phase may survive a
  body annotation. Diagnose the phase before trying more body annotations.
- Plain `cargo clean -p ... --target-dir ...` removed zero Flux artifacts in
  this environment. Do not infer freshness from issuing a clean command.
  Invalidating local Rust source fingerprints forced workspace rechecking while
  retaining third-party builds; the final run captured actual body witnesses.

The Firecracker run used six body annotations and three generated derive-site
trust entries. Its successful gate still allowed ordinary errors and dependent
targets that could not be reached. Neither the gate nor a reduced diagnostic
inventory establishes workspace coverage.
