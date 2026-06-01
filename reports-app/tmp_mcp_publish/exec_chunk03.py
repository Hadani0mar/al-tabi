#!/usr/bin/env python3
import json
import os
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
for env_path in (ROOT / ".env", ROOT.parent / ".env"):
    if not env_path.is_file():
        continue
    for line in env_path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, val = line.partition("=")
        key, val = key.strip(), val.strip().strip('"').strip("'")
        if key and key not in os.environ:
            os.environ[key] = val

key = os.environ.get("SUPABASE_SERVICE_ROLE_KEY", "").strip()
url = os.environ.get("SUPABASE_URL", "https://nsgmhijtaaenpqxxgjds.supabase.co").rstrip("/")
sql = Path(__file__).with_name("chunk_03.sql").read_text(encoding="utf-8")

if not key:
    raise SystemExit("SUPABASE_SERVICE_ROLE_KEY missing")

req = urllib.request.Request(
    f"{url}/rest/v1/rpc/exec_sql",
    data=json.dumps({"query": sql}).encode("utf-8"),
    method="POST",
    headers={
        "apikey": key,
        "Authorization": f"Bearer {key}",
        "Content-Type": "application/json",
    },
)
try:
    with urllib.request.urlopen(req, timeout=180) as resp:
        body = resp.read().decode("utf-8", errors="replace")
        print(f"SUCCESS HTTP {resp.status}: {body[:300]}")
except urllib.error.HTTPError as e:
    body = e.read().decode("utf-8", errors="replace")
    raise SystemExit(f"FAIL HTTP {e.code}: {body[:800]}")
