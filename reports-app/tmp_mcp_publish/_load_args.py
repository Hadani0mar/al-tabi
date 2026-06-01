import json
from pathlib import Path

root = Path(__file__).resolve().parent
args = json.loads(root.joinpath("_apply_args.json").read_text(encoding="utf-8"))
root.joinpath("_args_for_mcp.json").write_text(
    json.dumps(args, ensure_ascii=False), encoding="utf-8"
)
print("written", len(args["query"]))
