use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::{Component, EventDef, LlmSettings, PropDef, SlotDef};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiEnrichment {
    pub description: Option<String>,
    pub docs: Option<String>,
    pub tags: Vec<String>,
    pub props: Vec<PropDef>,
    pub events: Vec<EventDef>,
    pub slots: Vec<SlotDef>,
    pub examples: Vec<AiExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiExample {
    pub title: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiFetchPlan {
    pub urls: Vec<String>,
    pub note: Option<String>,
    /// Suggested library display name
    #[serde(default)]
    pub library_name: Option<String>,
}

/// Enrich a component from source using an OpenAI-compatible chat API.
pub async fn enrich_component(
    llm: &LlmSettings,
    component: &Component,
    source: &str,
) -> anyhow::Result<AiEnrichment> {
    let api_key = llm
        .api_key
        .as_deref()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("LLM API key not configured (admin settings)"))?;

    let system = r#"你是前端组件库分析助手。根据用户提供的组件源码，输出严格 JSON（不要 markdown 代码块），字段：
{
  "description": "一句话中文描述",
  "docs": "markdown 文档，含用法说明",
  "tags": ["标签"],
  "props": [{"name":"","type":"","default":"","required":false,"description":""}],
  "events": [{"name":"","payload":"","description":""}],
  "slots": [{"name":"","description":""}],
  "examples": [{"title":"基本用法","code":"..."}]
}
若某字段无法确定可省略或给空数组。优先补充缺失的 description / docs / examples / prop 说明。"#;

    let user = format!(
        "组件名: {}\n框架: {}\n路径: {}\n\n源码:\n```\n{}\n```",
        component.name,
        component.framework,
        component.file_path,
        truncate(source, 12000)
    );

    let content = chat_completion(llm, api_key, system, &user).await?;
    parse_enrichment(&content)
}

/// Suggest a short library display name from uploaded component names / files.
pub async fn suggest_library_name(
    llm: &LlmSettings,
    component_names: &[String],
    file_paths: &[String],
) -> anyhow::Result<String> {
    let api_key = llm
        .api_key
        .as_deref()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("LLM API key not configured (admin settings)"))?;

    let system = r#"你是命名助手。根据已入库的前端组件，给出一个简短的组件库中文或中英名称。
输出严格 JSON：{"library_name":"名称"}
规则：8～20 字内，可读、具体，不要空泛词如「未命名」「组件库1」。"#;

    let user = format!(
        "组件名: {}\n文件: {}",
        component_names.join(", "),
        file_paths.join(", ")
    );
    let content = chat_completion(llm, api_key, system, &user).await?;
    let cleaned = strip_fence(&content);
    let v: serde_json::Value = serde_json::from_str(&cleaned)
        .map_err(|e| anyhow::anyhow!("invalid name JSON: {e}"))?;
    let name = v["library_name"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("AI did not return library_name"))?;
    Ok(name.chars().take(40).collect())
}

/// Given a natural-language request, ask the model for raw component file URLs to download.
pub async fn plan_fetch_urls(llm: &LlmSettings, prompt: &str) -> anyhow::Result<AiFetchPlan> {
    let api_key = llm
        .api_key
        .as_deref()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("LLM API key not configured (admin settings)"))?;

    let system = r#"你是组件获取助手。用户只用自然语言描述想要的前端组件，你必须自行确定可下载的原始文件 URL，并起一个合适的组件库名称。
输出严格 JSON（不要 markdown）：
{"urls":["https://raw.githubusercontent.com/.../Button.vue"],"note":"说明","library_name":"Element Plus Button"}
规则：
1. urls 必须是可直接 GET 下载的原始文件地址（优先 raw.githubusercontent.com、cdn.jsdelivr.net/gh、unpkg.com）
2. 只返回组件相关文件：.vue / .uvue / .tsx / .jsx / .ts / .js，以及同目录 README.md（可选）
3. 不要返回 HTML 文档页、npm 主页、blob 页面
4. 根据用户描述选择最合适的开源组件库与路径；若不确定，给出最可能的 raw URL 并在 note 说明假设
5. 最多 15 个 url，优先主组件文件与直接依赖的样式/子组件
6. library_name 必填：8～24 字，概括来源与组件，例如「Element Plus Button」"#;

    let user = format!("用户需求:\n{prompt}");
    let content = chat_completion(llm, api_key, system, &user).await?;
    parse_fetch_plan(&content)
}

async fn chat_completion(
    llm: &LlmSettings,
    api_key: &str,
    system: &str,
    user: &str,
) -> anyhow::Result<String> {
    let url = format!("{}/chat/completions", llm.base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let body = json!({
        "model": llm.model,
        "temperature": 0.2,
        "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("LLM request failed ({status}): {text}");
    }

    let data: serde_json::Value = resp.json().await?;
    let content = data["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("LLM response missing content"))?
        .to_string();
    Ok(content)
}

fn parse_enrichment(content: &str) -> anyhow::Result<AiEnrichment> {
    let cleaned = strip_fence(content);
    let v: serde_json::Value = serde_json::from_str(&cleaned)
        .map_err(|e| anyhow::anyhow!("invalid AI JSON: {e}; raw={cleaned}"))?;

    let mut enrichment = AiEnrichment {
        description: v["description"].as_str().map(|s| s.to_string()),
        docs: v["docs"].as_str().map(|s| s.to_string()),
        tags: serde_json::from_value(v["tags"].clone()).unwrap_or_default(),
        props: serde_json::from_value(v["props"].clone()).unwrap_or_default(),
        events: serde_json::from_value(v["events"].clone()).unwrap_or_default(),
        slots: serde_json::from_value(v["slots"].clone()).unwrap_or_default(),
        examples: serde_json::from_value(v["examples"].clone()).unwrap_or_default(),
    };

    if enrichment
        .description
        .as_ref()
        .is_some_and(|s| s.trim().is_empty())
    {
        enrichment.description = None;
    }
    Ok(enrichment)
}

fn parse_fetch_plan(content: &str) -> anyhow::Result<AiFetchPlan> {
    let cleaned = strip_fence(content);
    let mut plan: AiFetchPlan = serde_json::from_str(&cleaned).unwrap_or(AiFetchPlan {
        urls: vec![],
        note: Some(cleaned.chars().take(200).collect()),
        library_name: None,
    });
    plan.urls
        .retain(|u| u.starts_with("http://") || u.starts_with("https://"));
    plan.urls.truncate(15);
    if plan.urls.is_empty() {
        anyhow::bail!("AI did not return any downloadable URLs");
    }
    if let Some(n) = plan.library_name.as_mut() {
        let t = n.trim().chars().take(40).collect::<String>();
        *n = t;
        if n.is_empty() {
            plan.library_name = None;
        }
    }
    Ok(plan)
}

fn strip_fence(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```json") {
        return rest
            .trim()
            .trim_start_matches('\n')
            .trim_end_matches("```")
            .trim()
            .to_string();
    }
    if let Some(rest) = t.strip_prefix("```") {
        return rest
            .trim()
            .trim_start_matches('\n')
            .trim_end_matches("```")
            .trim()
            .to_string();
    }
    t.to_string()
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

pub fn apply_enrichment(
    component: &mut Component,
    docs: &mut String,
    examples: &mut Vec<(String, String)>,
    enrichment: AiEnrichment,
) {
    if component
        .description
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        if let Some(d) = enrichment.description {
            component.description = Some(d);
        }
    }
    if docs.trim().is_empty() {
        if let Some(d) = enrichment.docs {
            *docs = d;
        }
    }
    for t in enrichment.tags {
        if !component.tags.iter().any(|x| x == &t) {
            component.tags.push(t);
        }
    }
    if component.props.is_empty() && !enrichment.props.is_empty() {
        component.props = enrichment.props;
    } else if !enrichment.props.is_empty() {
        for ep in enrichment.props {
            if let Some(p) = component.props.iter_mut().find(|p| p.name == ep.name) {
                if p.description
                    .as_ref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
                {
                    p.description = ep.description;
                }
                if p.r#type
                    .as_ref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
                {
                    p.r#type = ep.r#type;
                }
            }
        }
    }
    if component.events.is_empty() && !enrichment.events.is_empty() {
        component.events = enrichment.events;
    }
    if component.slots.is_empty() && !enrichment.slots.is_empty() {
        component.slots = enrichment.slots;
    }
    if examples.is_empty() {
        for ex in enrichment.examples {
            examples.push((ex.title, ex.code));
        }
    }
}
