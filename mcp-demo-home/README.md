# MCP 组件首页验证

用 CompiraMCP 工具链拉取组件，再组装成可运行的 Vue 首页。

## 流程

1. `list_libraries` / `search_components` 发现 Demo UI 组件
2. `get_component` 读取 Props / Slots
3. `get_component_source` 导出 SFC → `src/components/*.vue`
4. `validate_code` 校验 `prefer:Cmp` 规则
5. 本页直接 `import` 这些组件做交互

```bash
# 需 CompiraMCP 已在 :8080 运行
python ../scripts/mcp-export-components.py

npm install
npm run dev
# http://127.0.0.1:5174
```
