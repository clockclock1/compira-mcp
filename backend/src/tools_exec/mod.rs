use serde_json::Value;

use crate::db::Database;
use crate::mcp::validate_code_internal;

pub async fn execute_tool(
    db: &Database,
    tool: &str,
    args: Value,
) -> anyhow::Result<Value> {
    match tool {
        "search_components" => {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let library_id = args["library_id"].as_str().map(String::from);
            let limit = args["limit"].as_i64().unwrap_or(10);
            let results = db.search_components(&query, library_id.as_deref(), limit)?;
            Ok(serde_json::to_value(results)?)
        }
        "get_component" => {
            let id = args["component_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("component_id required"))?;
            let component = db
                .get_component(id)?
                .ok_or_else(|| anyhow::anyhow!("Component not found"))?;
            Ok(serde_json::to_value(component)?)
        }
        "get_component_source" => {
            let id = args["component_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("component_id required"))?;
            let source = db
                .get_component_source(id)?
                .ok_or_else(|| anyhow::anyhow!("Component not found"))?;
            Ok(serde_json::json!({ "source": source }))
        }
        "get_component_example" => {
            let id = args["component_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("component_id required"))?;
            let examples = db.get_component_examples(id)?;
            Ok(serde_json::to_value(examples)?)
        }
        "get_component_docs" => {
            let id = args["component_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("component_id required"))?;
            let docs = db.get_component_docs(id)?.unwrap_or_default();
            Ok(serde_json::json!({ "docs": docs }))
        }
        "search_source" => {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let library_id = args["library_id"].as_str().map(String::from);
            let limit = args["limit"].as_i64().unwrap_or(10);
            let results = db.search_source(&query, library_id.as_deref(), limit)?;
            Ok(serde_json::to_value(results)?)
        }
        "list_libraries" => {
            let libraries = db.list_libraries()?;
            Ok(serde_json::to_value(libraries)?)
        }
        "get_library_rules" => {
            let id = args["library_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("library_id required"))?;
            let library = db
                .get_library(id)?
                .ok_or_else(|| anyhow::anyhow!("Library not found"))?;
            Ok(serde_json::json!({
                "library_id": library.id,
                "name": library.name,
                "rules": library.rules.unwrap_or_else(|| "No specific rules defined.".into()),
                "component_count": library.component_count,
                "status": library.status,
            }))
        }
        "validate_code" => {
            let code = args["code"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("code required"))?;
            let library_id = args["library_id"].as_str();
            let violations = validate_code_internal(db, code, library_id)?;
            Ok(serde_json::json!({
                "valid": violations.is_empty(),
                "violations": violations,
            }))
        }
        _ => anyhow::bail!("Unknown tool: {tool}"),
    }
}

pub fn list_tool_names() -> Vec<&'static str> {
    vec![
        "search_components",
        "get_component",
        "get_component_source",
        "get_component_example",
        "get_component_docs",
        "search_source",
        "list_libraries",
        "get_library_rules",
        "validate_code",
    ]
}

pub fn tool_schemas() -> Value {
    serde_json::json!([
        { "name": "search_components", "description": "Search indexed frontend components", "parameters": { "query": "string", "library_id": "string?", "limit": "number?" } },
        { "name": "get_component", "description": "Get component metadata", "parameters": { "component_id": "string" } },
        { "name": "get_component_source", "description": "Get component source code", "parameters": { "component_id": "string" } },
        { "name": "get_component_example", "description": "Get component examples", "parameters": { "component_id": "string" } },
        { "name": "get_component_docs", "description": "Get component documentation", "parameters": { "component_id": "string" } },
        { "name": "search_source", "description": "Search in component source", "parameters": { "query": "string", "library_id": "string?", "limit": "number?" } },
        { "name": "list_libraries", "description": "List all libraries", "parameters": {} },
        { "name": "get_library_rules", "description": "Get library usage rules", "parameters": { "library_id": "string" } },
        { "name": "validate_code", "description": "Validate code against library rules", "parameters": { "code": "string", "library_id": "string?" } },
    ])
}
