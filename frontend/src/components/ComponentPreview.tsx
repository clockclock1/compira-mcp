import { useMemo } from "react";

type Props = {
  name: string;
  framework: string;
  source: string;
  exampleCode?: string;
};

/**
 * Sandboxed live preview for Vue SFC / React when source is available.
 */
export default function ComponentPreview({ name, framework, source, exampleCode }: Props) {
  const fw = framework.toLowerCase();

  const srcDoc = useMemo(() => {
    if (!source.trim()) return "";
    try {
      if (fw.includes("vue") || fw.includes("uni") || source.includes("<template")) {
        return buildVuePreview(source, exampleCode);
      }
      if (fw.includes("react") || /jsx|tsx/i.test(fw)) {
        return buildReactPreview(name, source, exampleCode);
      }
      return buildFallbackPreview(name, framework, source);
    } catch (e) {
      return buildErrorDoc(e instanceof Error ? e.message : String(e));
    }
  }, [name, framework, source, exampleCode, fw]);

  if (!source.trim()) {
    return <div className="empty">暂无源码，无法预览</div>;
  }

  return (
    <div className="preview-shell">
      <div className="preview-toolbar">
        <span className="chip-tag">{framework || "unknown"}</span>
        <span style={{ fontSize: 11.5, color: "var(--text-3)" }}>
          沙箱实时预览 · 含外部依赖的组件可能不完整
        </span>
      </div>
      <iframe
        className="preview-frame"
        title={`preview-${name}`}
        sandbox="allow-scripts"
        srcDoc={srcDoc}
      />
    </div>
  );
}

function esc(s: string) {
  return s.replace(/\\/g, "\\\\").replace(/`/g, "\\`").replace(/\$\{/g, "\\${");
}

function extractVueBlock(source: string, tag: string) {
  const re = new RegExp(`<${tag}[^>]*>([\\s\\S]*?)<\\/${tag}>`, "i");
  const m = source.match(re);
  return m ? m[1].trim() : "";
}

function buildVuePreview(source: string, exampleCode?: string) {
  const template =
    extractVueBlock(source, "template") ||
    `<div class="cmp-root"><p>No &lt;template&gt; found in source</p></div>`;
  const style = extractVueBlock(source, "style");
  const demo = (exampleCode || "").trim();
  const appTemplate = demo.includes("<")
    ? demo
    : `<div class="stage"><PreviewComp /></div>`;

  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8"/>
<style>
  html,body{margin:0;padding:0;background:#0b1220;color:#e8eef5;font-family:system-ui,sans-serif;}
  .stage{padding:20px;min-height:100vh;box-sizing:border-box;}
  .err{color:#fb7185;padding:16px;font-size:13px;white-space:pre-wrap;}
  ${style}
</style>
<script src="https://unpkg.com/vue@3/dist/vue.global.prod.js"><\/script>
</head>
<body>
<div id="app"></div>
<script>
try {
  const { createApp, defineComponent } = Vue;
  const PreviewComp = defineComponent({
    name: 'PreviewComp',
    template: \`${esc(template)}\`
  });
  createApp({
    components: { PreviewComp },
    template: \`${esc(appTemplate.includes("PreviewComp") || !demo.includes("<") ? appTemplate : `<div class="stage"><PreviewComp /></div>`)}\`
  }).mount('#app');
} catch (e) {
  document.body.innerHTML = '<div class="err">预览失败: ' + (e && e.message ? e.message : e) + '</div>';
}
<\/script>
</body>
</html>`;
}

function buildReactPreview(name: string, source: string, exampleCode?: string) {
  let code = source
    .replace(/^\s*import[\s\S]*?;?\s*$/gm, "")
    .replace(/export\s+default\s+/g, "const __PreviewComp = ")
    .replace(/export\s+/g, "");

  if (!code.includes("__PreviewComp")) {
    code += `\nconst __PreviewComp = (typeof ${name} !== 'undefined') ? ${name} : (() => React.createElement('div', null, ${JSON.stringify(name)}));`;
  }

  const hasDemo = !!(exampleCode || "").trim();
  const renderExpr = hasDemo
    ? `React.createElement('div', {className:'stage'}, React.createElement(__PreviewComp), React.createElement('pre', {style:{marginTop:16,opacity:0.7,fontSize:12}}, ${JSON.stringify(exampleCode)}))`
    : `React.createElement('div', {className:'stage'}, React.createElement(__PreviewComp))`;

  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8"/>
<style>
  html,body{margin:0;background:#0b1220;color:#e8eef5;font-family:system-ui,sans-serif;}
  .stage{padding:20px;}
  .err{color:#fb7185;padding:16px;white-space:pre-wrap;}
</style>
<script src="https://unpkg.com/react@18/umd/react.production.min.js"><\/script>
<script src="https://unpkg.com/react-dom@18/umd/react-dom.production.min.js"><\/script>
<script src="https://unpkg.com/@babel/standalone/babel.min.js"><\/script>
</head>
<body>
<div id="root"></div>
<script type="text/babel">
try {
${code}
const root = ReactDOM.createRoot(document.getElementById('root'));
root.render(${renderExpr});
} catch (e) {
  document.body.innerHTML = '<div class="err">预览失败: ' + (e && e.message ? e.message : e) + '</div>';
}
<\/script>
</body>
</html>`;
}

function buildFallbackPreview(name: string, framework: string, source: string) {
  return `<!DOCTYPE html><html><body style="margin:0;background:#0b1220;color:#9BB0BE;font-family:system-ui;padding:20px">
  <h3 style="color:#eaf2f6">${name}</h3>
  <p>框架 ${framework || "unknown"} 暂不支持自动实时预览，下方为源码摘录。</p>
  <pre style="background:#111827;padding:12px;border-radius:8px;overflow:auto;max-height:70vh;font-size:12px;color:#c5d4de">${source
    .slice(0, 4000)
    .replace(/</g, "&lt;")}</pre>
</body></html>`;
}

function buildErrorDoc(msg: string) {
  return `<!DOCTYPE html><html><body style="background:#0b1220;color:#fb7185;padding:16px;font-family:system-ui">${msg.replace(
    /</g,
    "&lt;",
  )}</body></html>`;
}
