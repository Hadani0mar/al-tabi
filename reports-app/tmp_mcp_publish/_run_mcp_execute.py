import json
from pathlib import Path

args = json.loads(Path(__file__).with_name("_mcp_tool_args.json").read_text(encoding="utf-8"))
out = Path(__file__).with_name("_exec_payload.json")
out.write_text(
    json.dumps({"project_id": args["project_id"], "query": args["query"]}, ensure_ascii=False),
    encoding="utf-8",
)
print("written", out.stat().st_size)
