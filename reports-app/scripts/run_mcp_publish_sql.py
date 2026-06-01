#!/usr/bin/env python3
"""Execute generated SQL files via Supabase MCP-style REST (service role from env)."""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SQL_DIR = ROOT / "tmp_mcp_publish"
MANIFEST = SQL_DIR / "manifest.json"

SUPABASE_URL = os.environ.get("SUPABASE_URL", "https://nsgmhijtaaenpqxxgjds.supabase.co").rstrip("/")
SERVICE_KEY = os.environ.get("SUPABASE_SERVICE_ROLE_KEY", "").strip()


def load_env() -> None:
    for path in (ROOT / ".env", ROOT.parent / ".env"):
        if not path.is_file():
            continue
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, _, val = line.partition("=")
            key, val = key.strip(), val.strip().strip('"').strip("'")
            if key and key not in os.environ:
                os.environ[key] = val


def run_sql(sql: str) -> None:
    if not SERVICE_KEY:
        raise SystemExit("SUPABASE_SERVICE_ROLE_KEY missing — set in env or .env")
    req = urllib.request.Request(
        f"{SUPABASE_URL}/rest/v1/rpc/exec_sql",
        data=json.dumps({"query": sql}).encode("utf-8"),
        headers={
            "apikey": SERVICE_KEY,
            "Authorization": f"Bearer {SERVICE_KEY}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            body = resp.read().decode("utf-8", errors="replace")
            print(f"  OK ({resp.status}): {body[:120]}")
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", errors="replace")
        raise SystemExit(f"HTTP {e.code}: {body[:500]}")


def main() -> None:
    load_env()
    global SERVICE_KEY
    SERVICE_KEY = os.environ.get("SUPABASE_SERVICE_ROLE_KEY", "").strip()

    if not MANIFEST.is_file():
        raise SystemExit(f"Run gen_mcp_publish_sql.py first — missing {MANIFEST}")

    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    files = [SQL_DIR / p["file"] for p in manifest.get("patterns", [])]
    files += [SQL_DIR / b["file"] for b in manifest.get("bundles", [])]

    for path in files:
        if not path.is_file():
            print(f"skip missing {path.name}")
            continue
        sql = path.read_text(encoding="utf-8")
        print(f"publishing {path.name} ({len(sql)} bytes)...")
        run_sql(sql)

    print("done")


if __name__ == "__main__":
    main()
