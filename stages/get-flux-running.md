# Get Flux running

Input: a target repository path.
Output: a repeatable command to run Flux, raw compiler logs, and Claude's native
session transcript. Claude decides what to investigate; there is no orchestrator.

1. Read the target's instructions and Cargo/toolchain configuration. Start at the
   repository root and enable Flux there with `[package.metadata.flux] enabled =
   true` if it is a package. For a virtual workspace, put package opt-in metadata
   in member manifests, not a nonexistent root package. Begin broadly; defer
   panic call graphs, selective package checking, and bin/lib refinements until
   a benchmark exposes a concrete problem. Record actual command/configuration
   changes in the transcript rather than designing selection heuristics now.
2. Initialize vendor/flux with `git submodule update --init`. Read its installation
   instructions, toolchain file, and CI. Build/install that exact revision with
   its required solver dependencies. Record setup commands, versions, executable
   paths, and required environment in the transcript. Global installs may replace
   an existing Flux; prefer an isolated environment. Do not substitute whichever
   cargo-flux happens to be on PATH. Make target edits in an isolated checkout.
3. Build fourgate (`cargo build --locked`). Capture each Flux attempt with:

   `fourgate record --repo /absolute/target --output /absolute/new-attempt-dir -- cargo flux --message-format=json`

   Use the built binary's absolute path. Create the output directory's parent
   first; the recorder creates the final directory and refuses to overwrite it.
   Keep logs outside the target checkout. It writes invocation.json, stdout.log,
   stderr.log, and result.json, returning the command's exit code. It passes argv
   directly, without shell interpretation. It does not mark agent task boundaries
   or analyze diagnostics. A missing result.json means the attempt did not finish
   recording its result; do not infer success from partial logs.
4. Inspect both logs for panics before discussing diagnostics. Use a fresh Cargo
   build directory if needed to demonstrate Flux actually ran. Keep setup
   prerequisites and any dirty target patch alongside the recorded invocation.
   Explain the observed outcome in the conversation, linking the relevant files.
   No separate summary template or custom event log is required.

Claude's raw JSONL transcript is the agent history. Native transcripts normally
live at `~/.claude/projects/<project>/<session-id>.jsonl`; storage can be configured.
Keep persistence enabled and preserve the session ID/path. When archiving a run,
copy that session's JSONL unchanged into the run directory before retention
cleanup. Do not sweep unrelated sessions or replace JSONL with a rendered export.
This is an archival step, not a new transcript parser or automatic fourgate hook.
See [Claude session documentation](https://code.claude.com/docs/en/sessions#where-transcripts-are-stored).

Done: pinned Flux runs on the codebase from the repository root, and another agent
can rerun it from the saved command and prerequisites. Refinement errors or ICEs
do not by themselves block this setup step. Setup failures that prevent Flux from running
are blockers; report them without claiming completion. A zero exit alone is not
proof Flux ran. CI checks formatting and Clippy only; it does not evaluate this
agent workflow.

Do not fix all errors, add trust attributes, or claim per-body coverage. Later
proof work can introduce new ICEs and require revisiting earlier exemptions.
