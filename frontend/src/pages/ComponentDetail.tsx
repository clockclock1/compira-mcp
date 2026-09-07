import { Link, useParams } from "react-router-dom";
import { useEffect, useState } from "react";
import { api, Component, ComponentExample } from "../api/client";
import ComponentPreview from "../components/ComponentPreview";
import { copyToClipboard } from "../lib/clipboard";

type Tab = "preview" | "docs" | "props" | "events" | "slots" | "examples" | "source";

export default function ComponentDetail() {
  const { id } = useParams<{ id: string }>();
  const [component, setComponent] = useState<Component | null>(null);
  const [source, setSource] = useState("");
  const [docs, setDocs] = useState("");
  const [examples, setExamples] = useState<ComponentExample[]>([]);
  const [tab, setTab] = useState<Tab>("preview");
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!id) return;
    api.components
      .get(id)
      .then(async (c) => {
        setComponent(c);
        const [src, doc, ex] = await Promise.all([
          api.components.source(id).catch(() => ({ source: "" })),
          api.components.docs(id).catch(() => ({ docs: "" })),
          api.components.examples(id).catch(() => []),
        ]);
        setSource(src.source);
        setDocs(doc.docs);
        setExamples(ex);
      })
      .catch((e) => setError(e.message));
  }, [id]);

  if (!component && !error) {
    return <div className="loading">加载中...</div>;
  }

  if (error) return <p className="error">{error}</p>;
  if (!component) return null;

  const copyImport = async () => {
    const stmt = `import ${component.name} from '${component.file_path}'`;
    const ok = await copyToClipboard(stmt);
    if (ok) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    }
  };

  return (
    <div className="page">
      <div className="crumb">
        <Link to="/libraries">组件库</Link>
        <span className="sep">/</span>
        <Link to={`/libraries/${component.library_id}`}>详情</Link>
        <span className="sep">/</span>
        <span className="cur">{component.name}</span>
      </div>

      <div className="page-header" style={{ marginBottom: 16 }}>
        <div>
          <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 6 }}>
            <h1>{component.name}</h1>
            <span className="chip-type">{component.framework}</span>
          </div>
          <div style={{ fontSize: 12.5, color: "var(--text-3)", marginBottom: 4 }}>
            {component.description || "暂无描述"}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-4)" }}>
            路径 {component.file_path}
            {component.tags.length > 0 && ` · ${component.tags.join(" · ")}`}
          </div>
        </div>
        <div className="header-actions">
          <button className="btn btn-ghost" onClick={copyImport}>
            {copied ? "已复制" : "复制导入语句"}
          </button>
          <button
            className="btn btn-ghost"
            style={{ background: "var(--bg-hover)", borderColor: "transparent", color: "var(--primary-light)" }}
            onClick={() => setTab("preview")}
          >
            预览效果
          </button>
          <button className="btn btn-ghost" onClick={() => setTab("source")}>
            查看源码
          </button>
        </div>
      </div>

      <div className="tabs">
        {(
          [
            ["preview", "预览"],
            ["docs", "文档"],
            ["props", "Props"],
            ["events", "Events"],
            ["slots", "Slots"],
            ["examples", "示例"],
            ["source", "源码"],
          ] as const
        ).map(([key, label]) => (
          <div
            key={key}
            className={`tab-item${tab === key ? " active" : ""}`}
            onClick={() => setTab(key)}
          >
            {label}
          </div>
        ))}
      </div>

      {tab === "preview" && (
        <div className="card" style={{ marginBottom: 16 }}>
          <div className="card-header">
            <div className="card-title">实时预览</div>
            <span className="chip-file">{component.file_path}</span>
          </div>
          <div className="divider" />
          <div className="card-body" style={{ padding: 12 }}>
            <ComponentPreview
              name={component.name}
              framework={component.framework}
              source={source}
              exampleCode={examples[0]?.code}
            />
          </div>
        </div>
      )}

      {tab === "props" && (
        <div className="card" style={{ marginBottom: 16 }}>
          <div className="card-header">
            <div className="card-title">Props 定义</div>
            <span className="chip-count">{component.props.length}</span>
          </div>
          <div className="divider" />
          {component.props.length === 0 ? (
            <div className="empty">无 Props</div>
          ) : (
            <table className="table">
              <thead>
                <tr>
                  <th style={{ width: 150 }}>名称</th>
                  <th style={{ width: 300 }}>类型</th>
                  <th style={{ width: 130 }}>默认值</th>
                  <th style={{ width: 70 }}>必填</th>
                  <th>说明</th>
                </tr>
              </thead>
              <tbody>
                {component.props.map((p) => (
                  <tr key={p.name}>
                    <td>
                      <span className="prop-name">{p.name}</span>
                    </td>
                    <td>
                      <span className="prop-type">{p.type || "—"}</span>
                    </td>
                    <td>
                      <span className="prop-default">{p.default || "—"}</span>
                    </td>
                    <td>
                      <span className="prop-required">{p.required ? "✓" : "—"}</span>
                    </td>
                    <td>
                      <span className="prop-desc">{p.description || "—"}</span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {tab === "events" && (
        <div className="card">
          <div className="card-header">
            <div className="card-title">Events 事件</div>
            <span className="chip-count">{component.events.length}</span>
          </div>
          <div className="divider" />
          {component.events.length === 0 ? (
            <div className="empty">无 Events</div>
          ) : (
            <table className="table">
              <thead>
                <tr>
                  <th style={{ width: 150 }}>名称</th>
                  <th style={{ width: 320 }}>参数</th>
                  <th>说明</th>
                </tr>
              </thead>
              <tbody>
                {component.events.map((e) => (
                  <tr key={e.name}>
                    <td>
                      <span className="mono" style={{ fontSize: 12, color: "var(--text-1)" }}>
                        {e.name}
                      </span>
                    </td>
                    <td>
                      <span className="mono" style={{ fontSize: 10, color: "var(--purple)" }}>
                        {e.payload || "—"}
                      </span>
                    </td>
                    <td>
                      <span style={{ fontSize: 11, color: "var(--text-2)" }}>
                        {e.description || "—"}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {tab === "slots" && (
        <div className="card">
          <div className="card-header">
            <div className="card-title">Slots 插槽</div>
            <span className="chip-count">{component.slots.length}</span>
          </div>
          <div className="divider" />
          {component.slots.length === 0 ? (
            <div className="empty">无 Slots</div>
          ) : (
            <table className="table">
              <thead>
                <tr>
                  <th style={{ width: 150 }}>名称</th>
                  <th>说明</th>
                </tr>
              </thead>
              <tbody>
                {component.slots.map((s) => (
                  <tr key={s.name}>
                    <td>
                      <span className="mono" style={{ fontSize: 12, color: "var(--text-1)" }}>
                        {s.name}
                      </span>
                    </td>
                    <td>
                      <span style={{ fontSize: 11, color: "var(--text-2)" }}>
                        {s.description || "—"}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {tab === "docs" && (
        <div className="card">
          <div className="card-header">
            <div className="card-title">文档 Docs</div>
            <span className="chip-file">README</span>
          </div>
          <div className="divider" />
          <div className="card-body">
            <pre className="code-block" style={{ whiteSpace: "pre-wrap" }}>
              {docs || "暂无文档"}
            </pre>
          </div>
        </div>
      )}

      {tab === "examples" && (
        <div className="card">
          <div className="card-header">
            <div className="card-title">示例 Examples</div>
            <span className="chip-count">{examples.length}</span>
          </div>
          <div className="divider" />
          <div className="card-body" style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {examples.length === 0 ? (
              <div className="empty" style={{ padding: 20 }}>
                暂无示例
              </div>
            ) : (
              examples.map((ex) => (
                <div key={ex.id}>
                  <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-1)", marginBottom: 8 }}>
                    {ex.title}
                  </div>
                  <pre className="code-block">{ex.code}</pre>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {tab === "source" && (
        <div className="card">
          <div className="card-header">
            <div className="card-title">源码 Source</div>
            <span className="chip-file">{component.file_path}</span>
          </div>
          <div className="divider" />
          <div className="card-body">
            <pre className="code-block" style={{ maxHeight: "60vh", overflow: "auto" }}>
              {source || "暂无源码"}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}
