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
    /// "urls" = download listed files; "repo" = clone whole repository
    #[serde(default = "default_fetch_mode")]
    pub mode: String,
    #[serde(default)]
    pub urls: Vec<String>,
    pub note: Option<String>,
    /// Suggested library display name
    #[serde(default)]
    pub library_name: Option<String>,
    /// When mode=repo
    #[serde(default)]
    pub repo_url: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    /// Import every component file under the repo (default true for mode=repo)
    #[serde(default = "default_true")]
    pub whole_repo: bool,
}

fn default_fetch_mode() -> String {
    "urls".into()
}

fn default_true() -> bool {
    true
}

/// Detect a git hosting URL and whether the user wants the whole repo (no specific component).
/// Returns `(clone_url, branch, whole_repo)`.
pub fn detect_repo_intent(prompt: &str) -> Option<(String, String, bool)> {
    let prompt = prompt.trim();
    let re = regex::Regex::new(
        r#"(?i)https?://(?:www\.)?(github\.com|gitlab\.com|gitee\.com)/([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+?)(?:\.git)?(?:/(?:tree|blob)/([^?\s#]+))?"#,
    )
    .ok()?;
    let caps = re.captures(prompt)?;
    let host = caps.get(1)?.as_str();
    let owner = caps.get(2)?.as_str();
    let repo = caps.get(3)?.as_str().trim_end_matches(".git");
    let mut branch = "main".to_string();
    if let Some(rest) = caps.get(4) {
        // tree/dev or tree/dev/packages/...
        if let Some(b) = rest.as_str().split('/').next() {
            if !b.is_empty() {
                branch = b.to_string();
            }
        }
    }
    let repo_url = format!("https://{host}/{owner}/{repo}.git");

    let remainder = re.replace_all(prompt, " ");
    let leftover: String = remainder
        .split_whitespace()
        .filter(|t| {
            let l = t.to_lowercase();
            !matches!(
                l.as_str(),
                "git" | "github" | "gitlab" | "gitee" | "repo" | "仓库" | "组件库" | "拉取" | "导入" | "clone"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");

    let whole_keywords = [
        "全部", "所有", "整个", "整库", "批量", "全量", "所有组件", "全部组件",
        "whole", "all", "entire", "everything",
    ];
    let has_whole = whole_keywords.iter().any(|k| {
        leftover.to_lowercase().contains(&k.to_lowercase()) || prompt.to_lowercase().contains(&k.to_lowercase())
    });

    // Only URL (maybe plus 全部…) => whole repo. Named component => not whole.
    let whole = leftover.is_empty() || has_whole;
    Some((repo_url, branch, whole))
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
    plan_fetch_urls_with_context(llm, prompt, None).await
}

/// Re-plan after previous links failed; `failure_context` lists what was tried.
pub async fn plan_fetch_urls_with_context(
    llm: &LlmSettings,
    prompt: &str,
    failure_context: Option<&str>,
) -> anyhow::Result<AiFetchPlan> {
    let api_key = llm
        .api_key
        .as_deref()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("LLM API key not configured (admin settings)"))?;

    let system = r#"你是组件获取助手。用户用自然语言或仓库地址描述要获取的前端组件。
服务端不会自动换镜像或改分支：每个 URL 只尝试一次。下载失败后会把失败信息给你，由你另找可用直链。
输出严格 JSON（不要 markdown）：
{"mode":"urls","urls":["https://cdn.jsdelivr.net/gh/owner/repo@dev/path/Button.vue"],"note":"说明","library_name":"Element Plus Button"}
或整库导入：
{"mode":"repo","repo_url":"https://github.com/owner/repo.git","branch":"dev","whole_repo":true,"note":"整库导入","library_name":"Element Plus"}

规则：
1. 若用户只给了 GitHub/GitLab/Gitee 仓库地址、未指定具体组件名，或明确说「全部/所有组件/整库」，必须 mode=repo 且 whole_repo=true
2. 若指定了具体组件（如 Button、Tag），mode=urls，给出可直接 GET 的文件地址（raw.githubusercontent.com / jsDelivr / unpkg / GitLab·Gitee raw）
3. urls 只含组件源文件：.vue/.uvue/.tsx/.jsx/.svelte/.astro/.dart/.wxml/.ts/.js 及可选 .html/.css/.scss/.md；不要 HTML 文档页/blob 页；最多 30 个
4. Element Plus 分支用 dev（不是 main）
5. 若提供了「上次失败信息」：必须给出与失败列表不同的新链接（可换站点、换路径、换包源），禁止原样重复；不要只把同一 GitHub 路径改成镜像域名糊弄
6. library_name 必填：8～24 字"#;

    let user = if let Some(ctx) = failure_context.filter(|s| !s.trim().is_empty()) {
        format!(
            "用户需求:\n{prompt}\n\n上次失败信息（请另找其他可用直链，不要重复失败地址）:\n{ctx}"
        )
    } else {
        format!("用户需求:\n{prompt}")
    };
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
        mode: "urls".into(),
        urls: vec![],
        note: Some(cleaned.chars().take(200).collect()),
        library_name: None,
        repo_url: None,
        branch: None,
        whole_repo: true,
    });
    if plan.mode.is_empty() {
        plan.mode = if plan.repo_url.as_ref().is_some_and(|u| !u.is_empty()) {
            "repo".into()
        } else {
            "urls".into()
        };
    }
    plan.urls
        .retain(|u| u.starts_with("http://") || u.starts_with("https://"));
    plan.urls.truncate(30);
    if plan.mode == "repo" {
        if plan
            .repo_url
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            anyhow::bail!("AI mode=repo but repo_url is empty");
        }
    } else if plan.urls.is_empty() {
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
