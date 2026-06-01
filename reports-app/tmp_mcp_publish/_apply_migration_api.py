#!/usr/bin/env python3
"""Apply migration via Supabase Management API (same as MCP apply_migration)."""
import json
import os
import urllib.error
import urllib.request
from pathlib import Path

PROJECT_ID = "nsgmhijtaaenpqxxgjds"
NAME = "seed_agent_chunk_01"
SQL_PATH = Path(__file__).resolve().parent / "chunk_01.sql"

auth = os.environ.get("SUPABASE_AUTH_HEADER", "").strip()
if not auth:
    print("FAIL: SUPABASE_AUTH_HEADER not set")
    raise SystemExit(1)
if not auth.lower().startswith("bearer "):
    auth = f"Bearer {auth}"

query = SQL_PATH.read_text(encoding="utf-8")
body = json.dumps({"query": query, "name": NAME}).encode("utf-8")

req = urllib.request.Request(
    f"https://api.supabase.com/v1/projects/{PROJECT_ID}/database/migrations",
    data=body,
    headers={
        "Authorization": auth,
        "Content-Type": "application/json",
    },
    method="POST",
)

try:
    with urllib.request.urlopen(req, timeout=180) as resp:
        result = resp.read().decode("utf-8", errors="replace")
        print(f"SUCCESS HTTP {resp.status}")
        print(result[:1000])
except urllib.error.HTTPError as e:
    err = e.read().decode("utf-8", errors="replace")
    print(f"FAIL HTTP {e.code}")
    print(err[:2000])
    raise SystemExit(1)
