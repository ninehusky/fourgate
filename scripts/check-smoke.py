"""Validate the fixture's specific assertion diagnostic, never an error count."""
import json
from pathlib import Path
import sys

attempt = Path(sys.argv[1])
logs = [(attempt / name).read_text() for name in ("stdout.log", "stderr.log")]
if any("panicked" in log or "internal compiler error" in log.lower() for log in logs):
    raise SystemExit("Flux panicked; smoke evidence is inconclusive")
events = [json.loads(line) for line in (attempt / "events.jsonl").read_text().splitlines()]
if events[-1]["type"] != "command_finished" or events[-1]["exit_code"] in (None, 0):
    raise SystemExit("expected a completed failing invocation")
for log in logs:
    for line in log.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        diagnostic = event.get("message", {})
        if not isinstance(diagnostic, dict):
            continue
        code = diagnostic.get("code") or {}
        if code.get("code") != "E0999":
            continue
        if "assertion might fail" not in diagnostic.get("message", "").lower():
            continue
        if any(span.get("is_primary") and span.get("file_name", "").endswith("src/lib.rs")
               and span.get("line_start") == 2 for span in diagnostic.get("spans", [])):
            print("verified expected assertion diagnostic at src/lib.rs:2")
            raise SystemExit(0)
raise SystemExit("expected assertion diagnostic absent; inspect raw logs")
