import json
import os
import urllib.error
import urllib.request
from pathlib import Path

root = Path(__file__).resolve().parent
args = json.loads(root.joinpath("_args_for_mcp.json").read_text(encoding="utf-8"))

auth = os.environ.get("SUPABASE_AUTH_HEADER", "").strip()
if not auth.lower().startswith("bearer "):
    auth = f"Bearer {auth}" if auth else ""

body = json.dumps({"query": args["query"], "name": args["name"]}).encode("utf-8")
req = urllib.request.Request(
    f"https://api.supabase.com/v1/projects/{args['project_id']}/database/migrations",
    data=body,
    headers={
        "Authorization": auth,
        "Content-Type": "application/json",
        "User-Agent": "cursor-supabase-mcp/1.0",
    },
    method="POST",
)

try:
    with urllib.request.urlopen(req, timeout=180) as resp:
        result = resp.read().decode("utf-8", errors="replace")
        root.joinpath("_migration_result.txt").write_text(
            f"SUCCESS HTTP {resp.status}\n{result}", encoding="utf-8"
        )
        print("SUCCESS", resp.status)
except urllib.error.HTTPError as e:
    err = e.read().decode("utf-8", errors="replace")
    root.joinpath("_migration_result.txt").write_text(
        f"FAIL HTTP {e.code}\n{err}", encoding="utf-8"
    )
    print("FAIL", e.code, err[:500])
    raise SystemExit(1)
