import base64
import json
import re
import urllib.error
import urllib.request
from pathlib import Path

from cryptography.hazmat.primitives.ciphers.aead import AESGCM

KEY = b"ReportsApp-SecureKey-2026-v1.0!!"
SETTINGS = Path.home() / "AppData/Roaming/com.dell.reports-app/settings.json"


def decrypt_api_key() -> str:
    text = SETTINGS.read_text(encoding="utf-8")
    match = re.search(r'"groq_api_key":\s*"([^"]+)"', text)
    if not match:
        raise SystemExit("groq_api_key not found in settings.json")
    combined = base64.b64decode(match.group(1))
    return AESGCM(KEY).decrypt(combined[:12], combined[12:], None).decode()


def get(path: str, api_key: str):
    req = urllib.request.Request(
        f"https://openrouter.ai/api/v1/{path}",
        headers={"Authorization": f"Bearer {api_key}"},
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.status, json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        body = e.read().decode()
        try:
            parsed = json.loads(body)
        except json.JSONDecodeError:
            parsed = {"raw": body}
        return e.code, parsed


def main():
    api_key = decrypt_api_key()
    masked = api_key[:10] + "..." + api_key[-4:]
    print(f"API key loaded: {masked}\n")

    for path in ("key", "credits"):
        status, body = get(path, api_key)
        print(f"=== GET /api/v1/{path} -> HTTP {status} ===")
        print(json.dumps(body, indent=2, ensure_ascii=False))

        if path == "key" and status == 200 and isinstance(body.get("data"), dict):
            d = body["data"]
            remaining = d.get("limit_remaining")
            usage = d.get("usage")
            print(
                f"\nSummary: limit_remaining={remaining}, usage(all-time)={usage}, "
                f"daily={d.get('usage_daily')}, monthly={d.get('usage_monthly')}"
            )
        if path == "credits" and status == 200 and isinstance(body.get("data"), dict):
            d = body["data"]
            purchased = d.get("total_credits")
            used = d.get("total_usage")
            if purchased is not None and used is not None:
                print(
                    f"\nSummary: balance ~ {purchased - used:.4f} USD "
                    f"(purchased {purchased}, used {used})"
                )
        print()


if __name__ == "__main__":
    main()
