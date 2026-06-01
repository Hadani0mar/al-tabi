import json
from pathlib import Path

payload = json.loads(Path("_exec_payload.json").read_text(encoding="utf-8"))
Path("_mcp_query_only.json").write_text(json.dumps({"query": payload["query"]}, ensure_ascii=False), encoding="utf-8")
print("query_json_bytes", Path("_mcp_query_only.json").stat().st_size)
