# Stage 0: establish invocation

Input: a target repository path and, if supplied, package/features/target choices.
Output: a repeatable Flux invocation and a short evidence trail, even if it fails.

1. Inspect the target's instructions, Cargo workspace, toolchains, and existing
   Flux configuration. Record intended packages/features/target; do not silently
   choose a different package to obtain success.
2. Initialize vendor/flux with `git submodule update --init`. Read its installation
   instructions, toolchain file, and CI. Build/install that exact revision with
   its required solver dependencies. Record setup commands and versions. Global
   installs may replace an existing Flux; prefer an isolated environment and
   record the resulting executable paths. Do not substitute whatever cargo-flux
   happens to be on PATH.
3. Make target changes in an isolated checkout. Flux packages must opt in with
   `[package.metadata.flux] enabled = true`. Record any required manifest edits
   as a patch. An invocation that skips the intended package is not established.
4. Build fourgate (`cargo build --locked`). For each command attempt, use:

   `fourgate record /absolute/target /absolute/new-task-dir -- cargo flux ...`

   Use the built binary's absolute path. Create the task directory's parent first;
   the recorder creates the final directory and refuses to overwrite it. Keep
   logs outside the target checkout. One command is one task/attempt; link related
   task IDs in summaries. No shell interpretation is performed; invoke a shell
   explicitly only when needed. The recorder returns the command's exit code.
5. Inspect stdout.log and stderr.log for panics before discussing diagnostics.
   Capture package selection evidence, actual tool versions, absolute executable
   paths, required environment, and a rerunnable argv in summary.md. If Cargo is
   cached, use a fresh build directory to demonstrate the checker actually ran.
6. Write summary.md: objective, observed outcome, evidence paths, decisions and
   reasons, blockers, next action. Include target revision/dirty patch, Flux SHA,
   package/features/target/toolchain, and setup prerequisites. Append JSON events
   for material decisions using type, task_id, parent_task_id, stage, timestamp_ms,
   explanation, and evidence paths. Do not log private environment or credentials.

Done: the intended package demonstrably reaches pinned Flux, and another agent
can rerun the recorded invocation. Refinement errors or ICEs do not by themselves
fail stage 0. Build/setup failures that prevent Flux from running are blockers;
report them without claiming completion. A zero exit code alone is insufficient.

Do not fix all errors, add trust attributes, or claim per-body coverage. Later
proof work can introduce new ICEs and require revisiting earlier exemptions.
