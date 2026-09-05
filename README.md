# CompiraMCP

远程前端组件库 **MCP** 服务：在 Web 后台管理组件库（Git / 上传 / AI 拉取），通过 [MCP Streamable HTTP](https://modelcontextprotocol.io/) 向 Cursor、Claude、Codex 等 Agent 提供**可检索的组件元数据、源码、文档与示例**。

仓库：<https://github.com/clockclock1/compira-mcp>  
镜像：`ghcr.io/clockclock1/compira-mcp`  
协议：Apache-2.0

---

## 目录

- [架构与技术](#架构与技术)
- [功能概览](#功能概览)
- [快速开始](#快速开始)
- [部署方式](#部署方式)
- [环境变量](#环境变量)
- [MCP 接入](#mcp-接入)
- [MCP 工具](#mcp-工具)
- [REST API](#rest-api)
- [组件入库](#组件入库)
- [CI / Release](#ci--release)
- [目录结构](#目录结构)

---

## 架构与技术

```
┌─────────────┐     REST + Session      ┌────────────────────────────────────┐
│ React Admin │ ◄──────────────────────► │  Rust Backend (Axum)               │
│ 管理后台    │                          │  ├── /api   REST（Bearer / API Key）│
└─────────────┘                          │  ├── /mcp   MCP Streamable HTTP    │
                                         │  ├── Git sync (libgit2 vendored)  │
┌─────────────┐   MCP HTTP + X-API-Key   │  ├── Vue / Uni-app 解析器         │
│ Cursor /    │ ◄──────────────────────► │  ├── SQLite + FTS5 全文索引       │
│ Claude/Codex│                          │  ├── 后台任务（同步 / 入库 / AI）  │
└─────────────┘                          │  └── OpenAI 兼容 LLM（可选）      │
                                         └────────────────────────────────────┘
```

| 层级 | 技术选型 | 说明 |
|------|----------|------|
| 后端 | Rust · Axum 0.8 · Tokio | 异步 HTTP；单二进制部署 |
| MCP | [rmcp](https://crates.io/crates/rmcp) Streamable HTTP | 标准 MCP 工具协议，`/mcp` |
| 存储 | SQLite（rusqlite bundled）+ FTS5 | 零外部数据库依赖，组件全文检索 |
| Git | git2（vendored-libgit2 / OpenSSL） | 跨平台克隆 / 拉取，不依赖系统 libgit2 |
| 解析 | 自研 Vue SFC / script 解析 | Props、Emits、Slots、defineModel、文档与示例 |
| 前端 | React · TypeScript · Vite | 管理后台；生产由后端静态托管 |
| 鉴权 | Session（登录）+ Argon2 · API Key（MCP） | UI 用 Bearer Token；Agent 用 `X-API-Key` |
| AI | OpenAI 兼容 Chat Completions | 组件补全、自然语言拉取、自动命名 |

**数据流简述**：创建组件库 → 同步/上传/AI 拉取 → 解析入库 → FTS 索引 → Agent 经 MCP `search_components` / `get_component_source` 取用。

---

## 功能概览

- 组件库：**Git 同步**、**本地上传**、**AI 自然语言拉取**
- 管理后台：仪表盘、组件详情、MCP 调试台、用户 / API Key、系统日志、LLM 设置
- MCP 九工具：搜索、元数据、源码、文档、示例、规则校验等
- 跨平台：Linux / Windows / macOS · x86_64 / arm64；Docker 多架构

---

## 快速开始

### 平台支持

| 运行方式 | Linux amd64 | Linux arm64 | Windows x64 | Windows arm64 | macOS Intel | macOS Apple Silicon |
|----------|:-----------:|:-----------:|:-----------:|:-------------:|:-----------:|:-------------------:|
| 源码 / Release 二进制 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Docker `linux/*` | ✅ | ✅ | ✅* | ✅* | ✅* | ✅* |

\* 桌面系统通过 Docker Desktop 运行 linux 容器。

构建依赖：Rust stable、Node 22+。Windows 编译 vendored OpenSSL 需 **MSVC** + **Strawberry Perl**。Windows 上若路径含非 ASCII，请将 `COMPIRA_DATA_DIR` 设为 ASCII 路径（如 `C:\compira-data`）。

### 本地开发（源码）

```bash
# 前端
cd frontend && npm install && npm run build && cd ..

# 后端（同时托管 Admin UI + MCP）
cd backend && cargo run

# 或脚本
./start.sh          # Linux / macOS release
./dev.sh            # 后端 debug + Vite :5173
start.bat / dev.bat # Windows
```

默认管理员：**admin / admin123**（请尽快修改）。控制台会打印首次 MCP Admin API Key。

访问 <http://localhost:8080>

---

## 部署方式

### 1. Docker Compose（推荐）

默认拉取 GHCR 多架构镜像（不本地 build），按宿主机自动选择 `linux/amd64` / `linux/arm64`：

```bash
git clone https://github.com/clockclock1/compira-mcp.git
cd compira-mcp
docker compose pull
docker compose up -d
```

数据卷：`compira-data` → 容器内 `/app/data`。  
访问 <http://localhost:8080>

需要指定版本时，可改 `docker-compose.yml` 中的 tag，例如 `ghcr.io/clockclock1/compira-mcp:v0.1.0`。

### 2. 直接 `docker run`

```bash
docker pull ghcr.io/clockclock1/compira-mcp:latest
# 或指定版本
docker pull ghcr.io/clockclock1/compira-mcp:v0.1.0

docker run -d --name compira-mcp \
  -p 8080:8080 \
  -v compira-data:/app/data \
  -e COMPIRA_ADMIN_PASSWORD='change-me' \
  ghcr.io/clockclock1/compira-mcp:latest
```

镜像由 Release 时的 Actions 构建并推送到 `ghcr.io/clockclock1/compira-mcp`。

### 3. Release 二进制包

在 [Releases](https://github.com/clockclock1/compira-mcp/releases) 下载对应平台产物：

| 文件 | 说明 |
|------|------|
| `compira-mcp-<os>-<arch>` / `.exe` | 可执行文件 |
| `compira-mcp-<os>-<arch>.tar.gz` / `.zip` | 含二进制 + `static/` 管理后台 |

```bash
# Linux 示例
tar -xzf compira-mcp-linux-amd64.tar.gz
export COMPIRA_STATIC_DIR="$PWD/static"
export COMPIRA_DATA_DIR="$PWD/data"
export COMPIRA_PORT=8080
./compira-mcp
```

```powershell
# Windows 示例
Expand-Archive compira-mcp-windows-amd64.zip -DestinationPath .
$env:COMPIRA_STATIC_DIR = "$PWD\static"
$env:COMPIRA_DATA_DIR = "C:\compira-data"
.\compira-mcp.exe
```

### 4. 源码编译部署

```bash
cd frontend && npm ci && npm run build && cd ..
cd backend && cargo build --release
export COMPIRA_STATIC_DIR="$(pwd)/../frontend/dist"
./target/release/compira-mcp
```

### 5. 反向代理（生产）

TLS 终止后反代到本服务即可，注意 MCP 可能使用较长连接：

```nginx
location / {
  proxy_pass http://127.0.0.1:8080;
  proxy_http_version 1.1;
  proxy_set_header Host $host;
  proxy_set_header X-Real-IP $remote_addr;
  proxy_read_timeout 3600s;
}
```

---

## 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `COMPIRA_HOST` | `0.0.0.0` | 监听地址 |
| `COMPIRA_PORT` | `8080` | 端口 |
| `COMPIRA_DATA_DIR` | `./data` | 数据库与仓库缓存目录 |
| `COMPIRA_DATABASE_URL` | `$DATA_DIR/compira.db` | SQLite 路径 |
| `COMPIRA_STATIC_DIR` | `../frontend/dist` | Admin UI 静态资源 |
| `COMPIRA_ADMIN_USERNAME` | `admin` | 首次引导管理员用户名 |
| `COMPIRA_ADMIN_PASSWORD` | `admin123` | 首次引导密码 |
| `COMPIRA_ADMIN_API_KEY` | — | 可选，引导写入的 MCP API Key |
| `COMPIRA_LLM_API_KEY` / `OPENAI_API_KEY` | — | LLM 种子 Key（可在「系统设置」覆盖） |
| `COMPIRA_LLM_BASE_URL` | `https://api.openai.com/v1` | OpenAI 兼容 Base URL |
| `COMPIRA_LLM_MODEL` | `gpt-4o-mini` | 模型名 |
| `RUST_LOG` | `info,compira_mcp=debug` | 日志级别 |

---

## MCP 接入

```json
{
  "mcpServers": {
    "compira": {
      "url": "http://localhost:8080/mcp",
      "headers": {
        "X-API-Key": "cmcp_xxxxxxxx"
      }
    }
  }
}
```

API Key 在管理后台「API Keys」创建。也可用登录后的 `Authorization: Bearer <session>` 调用 `/api/mcp/*` 调试接口。

---

## MCP 工具

| 工具 | 作用 |
|------|------|
| `search_components` | 按名称 / Props / Events / 描述全文搜索 |
| `get_component` | 组件元数据（props / events / slots / tags） |
| `get_component_source` | 完整源码 |
| `get_component_example` | 使用示例 |
| `get_component_docs` | 文档（同目录 md / README） |
| `search_source` | 在源码中关键词搜索 |
| `list_libraries` | 列出组件库 |
| `get_library_rules` | 库级使用规则 |
| `validate_code` | 按规则校验生成代码（`forbid:` / `require:` / `prefer:`） |

联调：

```powershell
$env:COMPIRA_API_KEY = "cmcp_xxxxxxxx"
.\scripts\mcp-test.ps1 -Query "Button"
```

浏览器：**MCP 调试** 页 → `POST /api/mcp/call`。

---

## REST API

除 `/api/health` 外，需认证：

- **管理后台**：`Authorization: Bearer <login token>`
- **MCP / 机器调用**：`X-API-Key: cmcp_...`（部分只读接口亦可用 Key）

基址：`http://<host>:8080/api`

### 健康与统计

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/health` | `{ "status": "ok", "service": "CompiraMCP" }` |
| `GET` | `/stats` | 库 / 组件 / 用户 / API Key 计数 |

### 认证与用户

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/auth/login` | `{ "username", "password" }` → `{ token, user }` |
| `POST` | `/auth/logout` | 注销会话 |
| `GET` | `/auth/me` | 当前用户 |
| `GET` / `POST` | `/users` | 用户列表 / 创建（admin） |
| `PATCH` / `DELETE` | `/users/{id}` | 更新 / 删除 |

### 组件库

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/libraries` | 列表 |
| `POST` | `/libraries` | 创建：`{ name, source_type: git\|upload\|fetch, repo_url?, branch?, rules? }` |
| `GET` | `/libraries/{id}` | 详情 |
| `PATCH` | `/libraries/{id}` | 更新 name / branch / rules |
| `DELETE` | `/libraries/{id}` | 删除库与本地缓存 |
| `POST` | `/libraries/{id}/sync` | 触发 Git 同步 → `{ task_id }` |
| `POST` | `/libraries/{id}/upload` | multipart：`files` + `use_ai` + `auto_name` → 202 |
| `POST` | `/libraries/{id}/fetch` | `{ "prompt", "auto_name"? }` AI 拉取 → 202 |
| `GET` | `/libraries/{id}/components` | 该库组件列表 |

### 组件与搜索

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/components/{id}` | 元数据 |
| `GET` | `/components/{id}/source` | `{ "source": "..." }` |
| `GET` | `/components/{id}/docs` | `{ "docs": "..." }` |
| `GET` | `/components/{id}/examples` | 示例数组 |
| `GET` | `/search/components?q=&library_id=&limit=` | FTS 搜索 |

### 任务 / AI / 设置

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/tasks` / `/tasks/{id}` | 同步与入库进度 |
| `GET` | `/ai/status` | LLM 是否可用（脱敏） |
| `GET` / `PUT` | `/settings/llm` | 管理员读写 LLM 配置 |
| `GET` / `POST` | `/api-keys` | API Key 管理 |
| `DELETE` | `/api-keys/{id}` | 删除 Key |
| `GET` | `/logs` | 应用日志 |
| `GET` | `/mcp/tools` | MCP 工具 schema |
| `POST` | `/mcp/call` | `{ "tool", "arguments" }` 代理调用 |

**上传示例：**

```bash
curl -X POST "http://localhost:8080/api/libraries/<id>/upload" \
  -H "Authorization: Bearer <token>" \
  -F "use_ai=true" -F "auto_name=false" \
  -F "files=@CmpButton.vue"
```

**AI 拉取示例：**

```bash
curl -X POST "http://localhost:8080/api/libraries/<id>/fetch" \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"prompt":"从 Element Plus 拉取 Button 组件 Vue 源码","auto_name":true}'
```

---

## 组件入库

1. **Git**：`source_type=git` + `repo_url` → 同步 → 全量扫描 `.vue` / `.uvue` / `.tsx` / `.jsx`
2. **上传**：multipart 文件 → 解析；可选 AI 补文档 / 示例；名称可留空由 AI 命名
3. **AI 拉取**：自然语言 → LLM 规划 raw URL → 下载入库（需配置 LLM）

解析能力：`defineProps`（类型 / 运行时）、`withDefaults`、`defineEmits`、`defineModel`、`defineOptions`、JSDoc、具名 slot、邻接 `.md` / `examples/`。

库规则（每行一条）：

```
forbid:antd
require:@my-lib/
prefer:Cmp
```

---

## CI / Release

工作流对齐 [Failover-Proxy](https://github.com/clockclock1/Failover-Proxy/tree/main/.github/workflows)：

| Workflow | 触发 | 作用 |
|----------|------|------|
| `ci.yml` | PR / 手动 | 多 OS `cargo check` + 前端 build |
| `build-binaries.yml` | **Release published** | 6 平台可执行文件 + 含 `static/` 的压缩包，挂到 Release |
| `docker.yml` | **Release published** | 编译 linux amd64/arm64 → 构建并推送 `ghcr.io/.../compira-mcp` 多架构镜像 |

发布步骤：

```bash
git tag v0.1.0
git push origin v0.1.0
# 在 GitHub 创建 Release（publish）→ 自动构建产物与镜像
```

本地多架构构建脚本仍可用：`scripts/docker-build.sh` / `scripts/docker-build.ps1`。

---

## 目录结构

```
├── backend/                 # Rust：API / MCP / 解析 / Git / 索引
├── frontend/                # React 管理后台
├── mcp-demo-home/           # 用 MCP 导出组件搭建的验证首页示例
├── samples/demo-ui/         # 示例 Vue 组件
├── scripts/                 # 跨平台脚本、MCP 导出 / 联调
├── .github/workflows/       # CI + Release 二进制 + Docker
├── Dockerfile               # 本地 compose 多阶段构建
├── Dockerfile.release       # Release 镜像（预编译二进制）
├── docker-compose.yml
├── start.sh / start.bat
└── dev.sh / dev.bat
```

---

## License

Apache License 2.0 — 见 [LICENSE](./LICENSE)。
