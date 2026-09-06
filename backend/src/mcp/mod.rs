use std::sync::Arc;
use std::time::Instant;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::Database;
use crate::tools_exec::execute_tool;

pub mod activity;
pub mod validate;
pub use activity::{McpActivity, McpCaller, McpCallEvent};
pub use validate::validate_code_internal;

#[derive(Clone)]
pub struct CompiraMcpServer {
    db: Database,
    activity: Arc<McpActivity>,
    #[allow(dead_code)] // required by rmcp #[tool_router] macro
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchComponentsArgs {
    pub query: String,
    #[serde(default)]
    pub library_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ComponentIdArgs {
    pub component_id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchSourceArgs {
    pub query: String,
    #[serde(default)]
    pub library_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ValidateCodeArgs {
    pub code: String,
    #[serde(default)]
    pub library_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LibraryIdArgs {
    pub library_id: String,
}

fn default_limit() -> i64 {
    10
}

#[tool_router]
impl CompiraMcpServer {
    pub fn new(db: Database, activity: Arc<McpActivity>) -> Self {
        Self {
            db,
            activity,
            tool_router: Self::tool_router(),
        }
    }

    fn json_result(value: impl Serialize) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into()),
        )]))
    }

    async fn run_tool(&self, tool: &str, args: Value) -> Result<CallToolResult, McpError> {
        let caller = activity::current_caller_or(McpCaller::mcp(None, None));
        let id = self.activity.begin(tool, &args, &caller);
        let started = Instant::now();
        match execute_tool(&self.db, tool, args).await {
            Ok(value) => {
                self.activity.finish(&id, true, None, started);
                // Source tool: prefer raw text for agents when present.
                if tool == "get_component_source" {
                    if let Some(source) = value.get("source").and_then(|s| s.as_str()) {
                        return Ok(CallToolResult::success(vec![ContentBlock::text(
                            source.to_string(),
                        )]));
                    }
                }
                Self::json_result(value)
            }
            Err(e) => {
                let msg = e.to_string();
                self.activity.finish(&id, false, Some(msg.clone()), started);
                Err(McpError::internal_error(msg, None))
            }
        }
    }

    #[tool(description = "Search indexed frontend components by name, props, events, tags or description")]
    async fn search_components(
        &self,
        Parameters(args): Parameters<SearchComponentsArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run_tool(
            "search_components",
            serde_json::to_value(&args).unwrap_or_default(),
        )
        .await
    }

    #[tool(description = "Get component metadata including props, events and slots")]
    async fn get_component(
        &self,
        Parameters(args): Parameters<ComponentIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run_tool(
            "get_component",
            serde_json::to_value(&args).unwrap_or_default(),
        )
        .await
    }

    #[tool(description = "Get full source code of a component")]
    async fn get_component_source(
        &self,
        Parameters(args): Parameters<ComponentIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run_tool(
            "get_component_source",
            serde_json::to_value(&args).unwrap_or_default(),
        )
        .await
    }

    #[tool(description = "Get usage examples for a component")]
    async fn get_component_example(
        &self,
        Parameters(args): Parameters<ComponentIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run_tool(
            "get_component_example",
            serde_json::to_value(&args).unwrap_or_default(),
        )
        .await
    }

    #[tool(description = "Get documentation for a component")]
    async fn get_component_docs(
        &self,
        Parameters(args): Parameters<ComponentIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run_tool(
            "get_component_docs",
            serde_json::to_value(&args).unwrap_or_default(),
        )
        .await
    }

    #[tool(description = "Search inside component source code")]
    async fn search_source(
        &self,
        Parameters(args): Parameters<SearchSourceArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run_tool(
            "search_source",
            serde_json::to_value(&args).unwrap_or_default(),
        )
        .await
    }

    #[tool(description = "List all indexed component libraries")]
    async fn list_libraries(&self) -> Result<CallToolResult, McpError> {
        self.run_tool("list_libraries", serde_json::json!({})).await
    }

    #[tool(description = "Get coding rules / conventions for a library")]
    async fn get_library_rules(
        &self,
        Parameters(args): Parameters<LibraryIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run_tool(
            "get_library_rules",
            serde_json::to_value(&args).unwrap_or_default(),
        )
        .await
    }

    #[tool(description = "Validate frontend code against indexed component APIs")]
    async fn validate_code(
        &self,
        Parameters(args): Parameters<ValidateCodeArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run_tool(
            "validate_code",
            serde_json::to_value(&args).unwrap_or_default(),
        )
        .await
    }
}

#[tool_handler]
impl ServerHandler for CompiraMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "CompiraMCP provides remote frontend component libraries. \
                 Use search_components to find components, get_component for API details, \
                 get_component_source for implementation, and validate_code to check generated code."
                    .to_string(),
            )
    }
}

pub fn create_mcp_service(
    db: Database,
    activity: Arc<McpActivity>,
) -> rmcp::transport::streamable_http_server::StreamableHttpService<
    CompiraMcpServer,
    rmcp::transport::streamable_http_server::session::local::LocalSessionManager,
> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpService,
    };
    let db_clone = db.clone();
    let activity_clone = activity.clone();
    // rmcp defaults allowed_hosts to localhost only — remote IP/domain access gets HTTP 403,
    // which Cursor surfaces as "Needs authentication". API Key middleware already protects /mcp.
    let config = mcp_http_config();
    StreamableHttpService::new(
        move || Ok(CompiraMcpServer::new(db_clone.clone(), activity_clone.clone())),
        LocalSessionManager::default().into(),
        config,
    )
}

/// Build streamable HTTP config. `COMPIRA_MCP_ALLOWED_HOSTS=host1,host2:8088` or `*` / unset = any Host.
fn mcp_http_config() -> rmcp::transport::streamable_http_server::StreamableHttpServerConfig {
    use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;

    let mut config = StreamableHttpServerConfig::default();
    match std::env::var("COMPIRA_MCP_ALLOWED_HOSTS") {
        Ok(raw) => {
            let raw = raw.trim();
            if raw.is_empty() || raw == "*" {
                tracing::info!("MCP Host check disabled (COMPIRA_MCP_ALLOWED_HOSTS={raw:?})");
                config = config.disable_allowed_hosts();
            } else {
                let hosts: Vec<String> = raw
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                tracing::info!("MCP allowed Hosts: {hosts:?}");
                config = config.with_allowed_hosts(hosts);
            }
        }
        Err(_) => {
            tracing::info!(
                "MCP Host check disabled by default (set COMPIRA_MCP_ALLOWED_HOSTS to restrict)"
            );
            config = config.disable_allowed_hosts();
        }
    }
    config
}
