# -*- coding: utf-8 -*-
"""Pull component sources via CompiraMCP REST (same data as MCP tools)."""
import json
import urllib.request
from pathlib import Path

BASE = "http://127.0.0.1:8080"
OUT = Path(__file__).resolve().parents[1] / "mcp-demo-home" / "src" / "components"


def req(method: str, path: str, token: str | None = None, body: dict | None = None):
    data = None if body is None else json.dumps(body).encode("utf-8")
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    r = urllib.request.Request(f"{BASE}{path}", data=data, headers=headers, method=method)
    with urllib.request.urlopen(r, timeout=30) as resp:
        return json.loads(resp.read().decode("utf-8"))


def mcp_call(token: str, tool: str, arguments: dict):
    return req("POST", "/api/mcp/call", token, {"tool": tool, "arguments": arguments})


def main():
    login = req("POST", "/api/auth/login", body={"username": "admin", "password": "admin123"})
    token = login["token"]

    libs = mcp_call(token, "list_libraries", {})["result"]
    print("libraries:", [(l["name"], l["component_count"]) for l in libs])

    search = mcp_call(token, "search_components", {"query": "Cmp", "limit": 20})["result"]
    comps = [c for c in search if c["name"].startswith("Cmp")]
    # also ensure we have all five
    for name in ["CmpButton", "CmpInput", "CmpCard", "CmpSwitch", "CmpTag"]:
        if not any(c["name"] == name for c in comps):
            hit = mcp_call(token, "search_components", {"query": name, "limit": 5})["result"]
            for h in hit:
                if h["name"] == name:
                    comps.append(h)

    OUT.mkdir(parents=True, exist_ok=True)
    meta_dir = OUT.parent / "mcp-meta"
    meta_dir.mkdir(parents=True, exist_ok=True)

    catalog = []
    for c in comps:
        cid = c["id"]
        name = c["name"]
        meta = mcp_call(token, "get_component", {"component_id": cid})["result"]
        src = mcp_call(token, "get_component_source", {"component_id": cid})["result"]["source"]
        docs = mcp_call(token, "get_component_docs", {"component_id": cid})["result"]
        examples = mcp_call(token, "get_component_example", {"component_id": cid})["result"]

        path = OUT / f"{name}.vue"
        path.write_text(src, encoding="utf-8")
        (meta_dir / f"{name}.json").write_text(
            json.dumps({"meta": meta, "docs": docs, "examples": examples}, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        catalog.append(
            {
                "id": cid,
                "name": name,
                "props": meta.get("props", []),
                "events": meta.get("events", []),
                "slots": meta.get("slots", []),
                "description": meta.get("description"),
            }
        )
        print(f"exported {name}: props={len(meta.get('props', []))} bytes={len(src)}")

    rules = mcp_call(
        token,
        "get_library_rules",
        {"library_id": "8d4aca4c-51e6-4a9b-b837-3fca1cbb8532"},
    )["result"]
    (meta_dir / "catalog.json").write_text(
        json.dumps({"rules": rules, "components": catalog}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

    # validate a sample snippet that prefers Cmp*
    sample = '<CmpButton type="primary" label="Go" />'
    v = mcp_call(
        token,
        "validate_code",
        {"code": sample, "library_id": "8d4aca4c-51e6-4a9b-b837-3fca1cbb8532"},
    )["result"]
    print("validate_code:", v)


if __name__ == "__main__":
    main()
