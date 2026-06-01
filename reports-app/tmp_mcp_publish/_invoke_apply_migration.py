#!/usr/bin/env python3
"""Load apply_migration args and print result path for MCP invocation."""
import json
from pathlib import Path

ARGS = Path(__file__).resolve().parent / "_apply_args.json"
args = json.loads(ARGS.read_text(encoding="utf-8"))
print("READY", args["project_id"], args["name"], len(args["query"]))
