#!/usr/bin/env python3
import argparse
import json
import os
import re
from typing import Dict, Optional, Tuple

CAMEL_RE = re.compile(r"([a-z0-9])([A-Z])")


def camel_to_kebab(value: str) -> str:
    return CAMEL_RE.sub(r"\1-\2", value).lower()


def resolve_schema(schema: Dict, components: Dict) -> Dict:
    cur = schema or {}
    while isinstance(cur, dict) and "$ref" in cur:
        ref = cur["$ref"]
        if ref.startswith("#/components/schemas/"):
            name = ref.split("/")[-1]
            cur = components.get(name, {})
        else:
            break
    return cur if isinstance(cur, dict) else {}


def infer_type(schema: Dict, components: Dict) -> Tuple[str, bool, Optional[str], Optional[str]]:
    cur = resolve_schema(schema, components)
    if "anyOf" in cur or "oneOf" in cur:
        return "json", False, None, None

    stype = cur.get("type")
    sformat = cur.get("format")

    if stype == "array":
        items = cur.get("items") or {}
        item_type, _, _, _ = infer_type(items, components)
        return "array", True, item_type, sformat

    if not stype:
        return "json", False, None, None

    return stype, False, None, sformat


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate CLI command tree from Penpot OpenAPI.")
    parser.add_argument("--schema", default="schemas/penpot.openapi.json")
    parser.add_argument("--out", default="schemas/command_tree.json")
    parser.add_argument("--default-base-url", default=os.getenv("PENPOT_BASE_URL", "https://penpot.example.com"))
    parser.add_argument("--api-path", default="/api/main/methods")
    args = parser.parse_args()

    with open(args.schema, "r", encoding="utf-8") as f:
        data = json.load(f)

    paths = data.get("paths") or {}
    components = (data.get("components") or {}).get("schemas") or {}

    resources: Dict[str, Dict] = {}

    for method_name, entry in paths.items():
        if not isinstance(entry, dict):
            continue
        http_methods = entry
        method_spec = http_methods.get("post") or next(iter(http_methods.values()), {})
        request_body = method_spec.get("requestBody") or {}
        content = request_body.get("content") or {}
        app_json = content.get("application/json") or {}
        schema = resolve_schema(app_json.get("schema") or {}, components)
        properties = schema.get("properties") or {}
        required = set(schema.get("required") or [])

        args_list = []
        for prop_name, prop_schema in properties.items():
            stype, is_list, item_type, sformat = infer_type(prop_schema, components)
            arg = {
                "name": prop_name,
                "flag": camel_to_kebab(prop_name),
                "schema_type": stype,
                "item_type": item_type,
                "format": sformat,
                "required": prop_name in required,
                "list": is_list,
            }
            args_list.append(arg)

        args_list.sort(key=lambda x: x["name"])

        parts = method_name.split("-")
        if len(parts) == 1:
            op_name = parts[0]
            res_name = "misc"
        else:
            op_name = parts[0]
            res_name = "-".join(parts[1:])

        resource = resources.setdefault(res_name, {"name": res_name, "ops": []})
        resource["ops"].append({
            "name": op_name,
            "method": method_name,
            "args": args_list,
        })

    out_resources = sorted(resources.values(), key=lambda r: r["name"])
    for res in out_resources:
        res["ops"] = sorted(res["ops"], key=lambda o: o["name"])

    tree = {
        "version": 1,
        "default_base_url": args.default_base_url,
        "default_api_path": args.api_path,
        "resources": out_resources,
    }

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(tree, f, indent=2, sort_keys=True)

    print(args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
