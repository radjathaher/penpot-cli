#!/usr/bin/env python3
import argparse
import json
import os
import urllib.request


def resolve_openapi_url() -> str:
    direct = os.getenv("PENPOT_OPENAPI_URL")
    if direct:
        return direct
    base = os.getenv("PENPOT_BASE_URL", "https://penpot.example.com")
    return base.rstrip("/") + "/api/main/doc/openapi.json"


def main() -> int:
    parser = argparse.ArgumentParser(description="Fetch Penpot OpenAPI schema.")
    parser.add_argument("--out", default="schemas/penpot.openapi.json")
    parser.add_argument("--url", default=None)
    args = parser.parse_args()

    url = args.url or resolve_openapi_url()
    req = urllib.request.Request(
        url,
        headers={
            "accept": "application/json",
            "user-agent": "penpot-cli",
        },
    )
    with urllib.request.urlopen(req) as resp:
        data = json.load(resp)

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, sort_keys=True)
    print(args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
