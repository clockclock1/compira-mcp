import json
import urllib.request

BASE = "http://127.0.0.1:8080"
LIB = "8d4aca4c-51e6-4a9b-b837-3fca1cbb8532"


def req(method, path, token=None, body=None):
    data = None if body is None else json.dumps(body).encode()
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    r = urllib.request.Request(f"{BASE}{path}", data=data, headers=headers, method=method)
    with urllib.request.urlopen(r, timeout=20) as resp:
        return json.loads(resp.read().decode())


def main():
    token = req("POST", "/api/auth/login", body={"username": "admin", "password": "admin123"})[
        "token"
    ]
    good = """
<template>
  <CmpCard title="Home">
    <CmpInput v-model="email" />
    <CmpButton type="primary" label="Go" />
    <CmpTag type="success">ok</CmpTag>
  </CmpCard>
</template>
"""
    bad = "<el-button>x</el-button>"

    def call(tool, args):
        return req("POST", "/api/mcp/call", token, {"tool": tool, "arguments": args})["result"]

    print("good", call("validate_code", {"code": good, "library_id": LIB}))
    print("bad ", call("validate_code", {"code": bad, "library_id": LIB}))


if __name__ == "__main__":
    main()
