import json
from pathlib import Path

args = json.loads(Path(__file__).with_name("_mcp_args_chunk_04.json").read_text(encoding="utf-8"))
Path(__file__).with_name("_mcp_call_args.json").write_text(
    json.dumps(
        {
            "server": "plugin-supabase-supabase",
            "toolName": "apply_migration",
            "arguments": args,
        },
        ensure_ascii=False,
    ),
    encoding="utf-8",
)
print("args_ready", len(args["query"]))
