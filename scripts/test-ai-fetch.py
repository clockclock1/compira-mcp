# -*- coding: utf-8 -*-
"""E2E: add components via AI fetch."""
import json
import time
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"


def req(method, path, token=None, body=None, timeout=60):
    data = None if body is None else json.dumps(body, ensure_ascii=False).encode("utf-8")
    headers = {"Content-Type": "application/json; charset=utf-8"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    r = urllib.request.Request(f"{BASE}{path}", data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(r, timeout=timeout) as resp:
            raw = resp.read().decode("utf-8")
            return resp.status, json.loads(raw) if raw else {}
    except urllib.error.HTTPError as e:
        raw = e.read().decode("utf-8", errors="replace")
        try:
            payload = json.loads(raw)
        except Exception:
            payload = {"error": raw}
        return e.code, payload


def main():
    code, login = req("POST", "/api/auth/login", body={"username": "admin", "password": "admin123"})
    assert code == 200, login
    token = login["token"]
    print("[ok] login")

    code, ai = req("GET", "/api/ai/status", token)
    print(f"[ai] enabled={ai.get('enabled')} model={ai.get('model')} base={ai.get('base_url')}")
    if not ai.get("enabled"):
        print("[fail] LLM not configured — set API key in 系统设置")
        return 1

    # create fetch library with placeholder name; AI will rename
    code, lib = req(
        "POST",
        "/api/libraries",
        token,
        {
            "name": "处理中…",
            "source_type": "fetch",
        },
    )
    print(f"[lib] create status={code} id={lib.get('id')} name={lib.get('name')}")
    if code not in (200, 201):
        print(lib)
        return 1
    lib_id = lib["id"]

    prompt = (
        "从 Element Plus 官方仓库拉取 Tag 标签组件的 Vue 源码，"
        "优先使用 raw.githubusercontent.com/element-plus/element-plus 下 "
        "packages/components/tag/src 的 .vue 文件（不要 HTML 文档页）。"
    )
    code, fetch = req(
        "POST",
        f"/api/libraries/{lib_id}/fetch",
        token,
        {"prompt": prompt, "auto_name": True},
        timeout=30,
    )
    print(f"[fetch] status={code} body={fetch}")
    if code not in (200, 202):
        return 1
    task_id = fetch["task_id"]

    task = None
    for i in range(90):
        time.sleep(2)
        code, task = req("GET", f"/api/tasks/{task_id}", token)
        status = task.get("status")
        print(f"  [{i}] {status} {task.get('progress')}% {task.get('message')}")
        if status in ("completed", "failed"):
            break

    code, lib2 = req("GET", f"/api/libraries/{lib_id}", token)
    code, comps = req("GET", f"/api/libraries/{lib_id}/components", token)
    if not isinstance(comps, list):
        comps = []

    print("\n=== Result ===")
    print(f"library: {lib2.get('name')} status={lib2.get('status')} count={lib2.get('component_count')}")
    print(f"task: {task.get('status')} — {task.get('message')}")
    for c in comps:
        print(f"  - {c.get('name')}  props={len(c.get('props') or [])}  path={c.get('file_path')}")

    if task.get("status") != "completed" or not comps:
        print("[fail] AI fetch did not produce components")
        return 1

    # MCP search sanity
    code, mcp = req(
        "POST",
        "/api/mcp/call",
        token,
        {"tool": "search_components", "arguments": {"query": comps[0]["name"], "limit": 5}},
    )
    print(f"[mcp] search ok={mcp.get('ok')} hits={len(mcp.get('result') or [])}")
    print("[pass] AI add components works")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
