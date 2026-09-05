use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::{Deserialize, Serialize};

use crate::db::Database;

pub mod validate;
pub use validate::validate_code_internal;

#[derive(Clone)]
pub struct CompiraMcpServer {
    db: Database,
    #[allow(dead_code)] // required by rmcp #[tool_router] macro
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchComponentsArgs {
    pub query: String,
    #[serde(default)]
    pub library_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ComponentIdArgs {
    pub component_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSourceArgs {
    pub query: String,
    #[serde(default)]
    pub library_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ValidateCodeArgs {
    pub code: String,
    #[serde(default)]
    pub library_id: Option<String>,
}

fn default_limit() -> i64 {
    10
}

#[tool_router]
impl CompiraMcpServer {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    fn json_result(value: impl Serialize) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into()),
        )]))
    }

    #[tool(description = "Search indexed frontend components by name, props, events, tags or description")]
    async fn search_components(
        &self,
        Parameters(args): Parameters<SearchComponentsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let results = self
            .db
            .search_components(&args.query, args.library_id.as_deref(), args.limit)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Self::json_result(results)
    }

    #[tool(description = "Get component metadata including props, events and slots")]
    async fn get_component(
        &self,
        Parameters(args): Parameters<ComponentIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let component = self
            .db
            .get_component(&args.component_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Component not found", None))?;
        Self::json_result(component)
    }

    #[tool(description = "Get full source code of a component")]
    async fn get_component_source(
        &self,
        Parameters(args): Parameters<ComponentIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let source = self
            .db
            .get_component_source(&args.component_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Component not found", None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(source)]))
    }

    #[tool(description = "Get usage examples for a component")]
    async fn get_component_example(
        &self,
        Parameters(args): Parameters<ComponentIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let examples = self
            .db
            .get_component_examples(&args.component_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Self::json_result(examples)
    }

    #[tool(description = "Get documentation for a component")]
    async fn get_component_docs(
        &self,
        Parameters(args): Parameters<ComponentIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let docs = self
            .db
            .get_component_docs(&args.component_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .unwrap_or_default();
        Ok(CallToolResult::success(vec![ContentBlock::text(docs)]))
    }

    #[tool(description = "Search component source code for patterns or keywords")]
    async fn search_source(
        &self,
        Parameters(args): Parameters<SearchSourceArgs>,
    ) -> Result<CallToolResult, McpError> {
        let results = self
            .db
            .search_source(&args.query, args.library_id.as_deref(), args.limit)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Self::json_result(results)
    }

    #[tool(description = "List all registered component libraries")]
    async fn list_libraries(&self) -> Result<CallToolResult, McpError> {
        let libraries = self
            .db
            .list_libraries()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Self::json_result(libraries)
    }

    #[tool(description = "Get usage rules and constraints for a component library")]
    async fn get_library_rules(
        &self,
        Parameters(args): Parameters<LibraryIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let library = self
            .db
            .get_library(&args.library_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Library not found", None))?;
        Self::json_result(serde_json::json!({
            "library_id": library.id,
            "name": library.name,
            "rules": library.rules.unwrap_or_else(|| "No specific rules defined.".into()),
            "component_count": library.component_count,
            "status": library.status,
        }))
    }

    #[tool(description = "Validate generated code against component library rules and available components")]
    async fn validate_code(
        &self,
        Parameters(args): Parameters<ValidateCodeArgs>,
    ) -> Result<CallToolResult, McpError> {
        let violations = validate_code_internal(&self.db, &args.code, args.library_id.as_deref())
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Self::json_result(serde_json::json!({
            "valid": violations.is_empty(),
            "violations": violations,
        }))
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LibraryIdArgs {
    pub library_id: String,
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
) -> rmcp::transport::streamable_http_server::StreamableHttpService<
    CompiraMcpServer,
    rmcp::transport::streamable_http_server::session::local::LocalSessionManager,
> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpService,
    };
    let db_clone = db.clone();
    StreamableHttpService::new(
        move || Ok(CompiraMcpServer::new(db_clone.clone())),
        LocalSessionManager::default().into(),
        Default::default(),
    )
}

