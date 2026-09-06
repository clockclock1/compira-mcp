import { FormEvent, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, McpTool } from "../api/client";

const PRESETS: Record<string, object> = {
  list_libraries: {},
  search_components: { query: "Button", limit: 5 },
  search_source: { query: "defineProps", limit: 5 },
  get_component: { component_id: "" },
  get_component_source: { component_id: "" },
  get_component_docs: { component_id: "" },
  get_component_example: { component_id: "" },
  get_library_rules: { library_id: "" },
  validate_code: {
    code: '<MyButton label="OK" @click="handleClick" />',
    library_id: "",
  },
};

export default function McpPlayground() {
  const [tools, setTools] = useState<McpTool[]>([]);
  const [tool, setTool] = useState("list_libraries");
  const [argsJson, setArgsJson] = useState("{}");
  const [result, setResult] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [elapsed, setElapsed] = useState<number | null>(null);

  useEffect(() => {
    api.mcp
      .tools()
      .then((list) => {
        setTools(list);
        if (list.length && !list.find((t) => t.name === tool)) {
          setTool(list[0].name);
        }
      })
      .catch((e) => setError(e.message));
  }, []);

  useEffect(() => {
    setArgsJson(JSON.stringify(PRESETS[tool] ?? {}, null, 2));
  }, [tool]);

  const handleCall = async (e: FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError("");
    setResult("");
    setElapsed(null);
    const start = performance.now();
    try {
      const args = JSON.parse(argsJson);
      const res = await api.mcp.call(tool, args);
      setResult(JSON.stringify(res, null, 2));
      setElapsed(Math.round(performance.now() - start));
    } catch (err) {
      setError(err instanceof Error ? err.message : "调用失败");
      setElapsed(Math.round(performance.now() - start));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>MCP 调试</h1>
          <div className="page-sub">在线调用后端 MCP 工具，验证组件库数据服务</div>
        </div>
        <div className="header-actions">
          <Link to="/mcp/live" className="btn btn-ghost">
            实时监控
          </Link>
          <div className="chip-status chip-green">
            <span className="dot" />
            已连接 · Bearer Token
          </div>
        </div>
      </div>

      {error && <p className="error">{error}</p>}

      <div className="mcp-body">
        <div className="card">
          <div className="card-header">
            <div className="card-title">调用配置</div>
            <span className="chip-method">POST /api/mcp/call</span>
          </div>
          <div className="divider" />
          <form className="card-body" onSubmit={handleCall}>
            <div className="field">
              <label className="field-label">工具</label>
              <select
                className="tool-select"
                value={tool}
                onChange={(e) => setTool(e.target.value)}
              >
                {(tools.length ? tools.map((t) => t.name) : Object.keys(PRESETS)).map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
              {tools.find((t) => t.name === tool)?.description && (
                <div className="hint-text" style={{ marginTop: 6 }}>
                  {tools.find((t) => t.name === tool)?.description}
                </div>
              )}
            </div>
            <div className="field">
              <label className="field-label">参数 JSON</label>
              <textarea
                className="json-edit"
                rows={10}
                value={argsJson}
                onChange={(e) => setArgsJson(e.target.value)}
              />
              <div className="hint-text">参数以 JSON 格式发送</div>
            </div>
            <button
              type="submit"
              className="btn btn-primary"
              style={{ width: "100%", height: 40, fontSize: 14, justifyContent: "center" }}
              disabled={loading}
            >
              {loading ? "请求中…" : "发送请求"}
            </button>
          </form>
        </div>

        <div className="card">
          <div className="card-header">
            <div className="card-title">请求 / 响应</div>
          </div>
          <div className="divider" />
          <div className="card-body">
            <div className="resp-section">
              <div className="resp-label">请求 Request</div>
              <div className="req-code">
                <span className="method">POST</span> <span className="dim">/api/mcp/call</span>
                <br />
                <span className="dim">Authorization: Bearer ****</span>
                <br />
                <span className="dim">Content-Type: application/json</span>
                <br />
                <span className="dim">{`{ "tool": "${tool}", "arguments": … }`}</span>
              </div>
            </div>
            <div className="resp-section">
              <div className="resp-label">响应 Response</div>
              {elapsed !== null && (
                <div className="status-row">
                  <span className="ok-chip">{error ? "ERROR" : "200 OK"}</span>
                  <span className="status-meta">{elapsed}ms</span>
                </div>
              )}
              {result ? (
                <pre className="code-block" style={{ maxHeight: 420, overflow: "auto", whiteSpace: "pre-wrap" }}>
                  {result}
                </pre>
              ) : (
                <div className="empty" style={{ padding: 24 }}>
                  发送请求后在此查看结果
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
