use regex::Regex;
use std::path::Path;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::{Component, EventDef, PropDef, SlotDef};

const COMPONENT_EXTENSIONS: &[&str] = &["vue", "uvue", "tsx", "jsx", "ts", "js"];
const QUOTED: &str = r#"['"]([^'"]+)['"]"#;

fn capture_all(re: &Regex, text: &str) -> Vec<String> {
    re.captures_iter(text)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

pub fn is_component_extension(ext: &str) -> bool {
    COMPONENT_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

/// Parse a single component file relative to `root`.
pub fn parse_component_file(
    library_id: &str,
    root: &Path,
    relative_path: &str,
) -> anyhow::Result<(Component, String, String, Vec<(String, String)>)> {
    let path = root.join(relative_path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if !is_component_extension(&ext) {
        anyhow::bail!("unsupported component file: {relative_path}");
    }
    let source = std::fs::read_to_string(&path)?;
    let rel_path = relative_path.replace('\\', "/");
    let framework = detect_framework(root, &ext, &source);
    let parsed = if matches!(ext.as_str(), "vue" | "uvue") {
        parse_vue_component(&source, &rel_path, &framework)
    } else {
        parse_script_component(&source, &rel_path, &framework)
    };
    let docs = find_docs(&path);
    let examples = find_examples(&path, &parsed.name);
    let component = Component {
        id: Uuid::new_v4().to_string(),
        library_id: library_id.to_string(),
        name: parsed.name,
        file_path: rel_path,
        framework,
        description: parsed.description,
        props: parsed.props,
        events: parsed.events,
        slots: parsed.slots,
        tags: parsed.tags,
    };
    Ok((component, source, docs, examples))
}

fn detect_framework(root: &Path, ext: &str, source: &str) -> String {
    if ext == "uvue" || is_uniapp_project(root) {
        return "uni-app".into();
    }
    if matches!(ext, "tsx" | "jsx") || source.contains("React.") || source.contains("from \"react\"") || source.contains("from 'react'") {
        return "react".into();
    }
    if matches!(ext, "vue") {
        return "vue".into();
    }
    "js".into()
}

fn parse_script_component(source: &str, file_path: &str, framework: &str) -> ParsedComponent {
    let name = extract_component_name(source, file_path);
    let description = extract_description(source, source);
    let mut tags = vec![framework.to_string()];
    if source.contains("export default") || source.contains("export function") {
        tags.push("module".into());
    }
    ParsedComponent {
        name,
        description,
        props: vec![],
        events: vec![],
        slots: vec![],
        tags,
    }
}

pub fn scan_and_parse_directory(
    library_id: &str,
    root: &Path,
) -> anyhow::Result<Vec<(Component, String, String, Vec<(String, String)>)>> {
    let mut results = Vec::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();

        if !COMPONENT_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        if should_skip(path) {
            continue;
        }

        let source = std::fs::read_to_string(path)?;
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let framework = if is_uniapp_project(root) || ext == "uvue" {
            "uni-app".to_string()
        } else if matches!(ext.as_str(), "tsx" | "jsx") {
            "react".to_string()
        } else if ext == "vue" {
            "vue".to_string()
        } else {
            detect_framework(root, &ext, &source)
        };

        let parsed = if matches!(ext.as_str(), "vue" | "uvue") {
            parse_vue_component(&source, &rel_path, &framework)
        } else {
            parse_script_component(&source, &rel_path, &framework)
        };
        let docs = find_docs(path);
        let examples = find_examples(path, &parsed.name);

        let component = Component {
            id: Uuid::new_v4().to_string(),
            library_id: library_id.to_string(),
            name: parsed.name,
            file_path: rel_path,
            framework: framework.to_string(),
            description: parsed.description,
            props: parsed.props,
            events: parsed.events,
            slots: parsed.slots,
            tags: parsed.tags,
        };

        results.push((component, source, docs, examples));
    }

    Ok(results)
}

fn should_skip(path: &Path) -> bool {
    let s = path.to_string_lossy().to_lowercase();
    s.contains("node_modules")
        || s.contains("/dist/")
        || s.contains("\\dist\\")
        || s.contains("/.git/")
        || s.contains("\\test\\")
        || s.contains("/test/")
        || s.contains("__tests__")
        || s.contains(".spec.")
        || s.contains(".test.")
}

fn is_uniapp_project(root: &Path) -> bool {
    root.join("pages.json").exists() || root.join("manifest.json").exists()
}

struct ParsedComponent {
    name: String,
    description: Option<String>,
    props: Vec<PropDef>,
    events: Vec<EventDef>,
    slots: Vec<SlotDef>,
    tags: Vec<String>,
}

fn parse_vue_component(source: &str, file_path: &str, framework: &str) -> ParsedComponent {
    let script = extract_script(source);
    let name = extract_component_name(&script, file_path);
    let description = extract_description(source, &script);
    let mut props = extract_props(&script);
    props = merge_with_defaults(&script, props);
    props = apply_jsdoc_props(&script, props);
    let mut events = extract_events(&script);
    events = apply_jsdoc_events(&script, events);
    let slots = extract_slots(source);
    let mut tags = vec![framework.to_string()];
    if script.contains("defineOptions") || script.contains("script setup") {
        tags.push("script-setup".into());
    }
    if source.contains("<template") {
        tags.push("template".into());
    }
    if script.contains("defineModel") {
        tags.push("v-model".into());
    }
    if framework == "uni-app" {
        tags.push("mobile".into());
    }

    ParsedComponent {
        name,
        description,
        props,
        events,
        slots,
        tags,
    }
}

fn extract_script(source: &str) -> String {
    Regex::new(r"(?s)<script[^>]*>(.*)</script>")
        .ok()
        .and_then(|re| re.captures(source))
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| source.to_string())
}

fn extract_component_name(script: &str, file_path: &str) -> String {
    if let Some(caps) = Regex::new(r#"name\s*:\s*['"]([^'"]+)['"]"#)
        .ok()
        .and_then(|re| re.captures(script))
    {
        return caps[1].to_string();
    }
    if let Some(caps) = Regex::new(r#"defineOptions\s*\(\s*\{\s*name\s*:\s*['"]([^'"]+)['"]"#)
        .ok()
        .and_then(|re| re.captures(script))
    {
        return caps[1].to_string();
    }

    Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string()
}

fn extract_description(source: &str, script: &str) -> Option<String> {
    if let Some(caps) = Regex::new(r#"@description\s+(.+)"#)
        .ok()
        .and_then(|re| re.captures(script))
    {
        return Some(caps[1].trim().to_string());
    }
    if let Some(caps) = Regex::new(r#"@component\s+(.+)"#)
        .ok()
        .and_then(|re| re.captures(script))
    {
        return Some(caps[1].trim().to_string());
    }
    if let Some(caps) = Regex::new(r#"<!--\s*@component\s+(.+?)\s*-->"#)
        .ok()
        .and_then(|re| re.captures(source))
    {
        return Some(caps[1].trim().to_string());
    }
    // First line of block comment in script
    if let Some(caps) = Regex::new(r"(?s)/\*\*\s*\n\s*\*\s*(.+?)\n")
        .ok()
        .and_then(|re| re.captures(script))
    {
        let desc = caps[1].trim().trim_start_matches('*').trim();
        if !desc.starts_with('@') && !desc.is_empty() {
            return Some(desc.to_string());
        }
    }
    None
}

fn extract_props(script: &str) -> Vec<PropDef> {
    let mut props = Vec::new();

    // defineModel(name, options?) — Vue 3.4+
    if let Ok(re) = Regex::new(r#"defineModel\s*(?:<[^>]+>)?\s*\(\s*['"](\w+)['"]"#) {
        for name in capture_all(&re, script) {
            props.push(PropDef {
                name,
                r#type: Some("string | number".into()),
                default: None,
                required: false,
                description: Some("v-model binding".into()),
            });
        }
    }
    if script.contains("defineModel()") || script.contains("defineModel<>()") {
        props.push(PropDef {
            name: "modelValue".into(),
            r#type: Some("any".into()),
            default: None,
            required: false,
            description: Some("default v-model".into()),
        });
    }

    if let Some(block) = extract_balanced_block(script, "defineProps<", ">") {
        props.extend(parse_ts_interface_props(&block));
    } else if let Some(block) = extract_balanced_block(script, "defineProps({", "})") {
        props.extend(parse_object_props_detailed(&block));
    } else if let Some(block) = extract_balanced_block(script, "defineProps([", "])") {
        props.extend(parse_array_props(&block));
    }

    if let Some(block) = extract_props_option(script) {
        props.extend(parse_object_props_detailed(&block));
    }

    // PropType<{ ... }> style in Options API
    if let Ok(re) = Regex::new(r"(\w+)\s*:\s*\{\s*type\s*:\s*([^,}]+)") {
        for cap in re.captures_iter(script) {
            props.push(PropDef {
                name: cap[1].to_string(),
                r#type: Some(cap[2].trim().to_string()),
                default: None,
                required: false,
                description: None,
            });
        }
    }

    dedupe_props(props)
}

fn merge_with_defaults(script: &str, mut props: Vec<PropDef>) -> Vec<PropDef> {
    let defaults_block = extract_balanced_block(script, "withDefaults(defineProps", ")")
        .or_else(|| extract_balanced_block(script, "withDefaults( defineProps", ")"));

    if let Some(block) = defaults_block {
        if let Ok(re) = Regex::new(r"(\w+)\s*:\s*([^,\n}]+)") {
            for cap in re.captures_iter(&block) {
                let name = cap[1].to_string();
                let default_val = cap[2].trim().trim_end_matches(',').to_string();
                if let Some(p) = props.iter_mut().find(|p| p.name == name) {
                    p.default = Some(default_val);
                    p.required = false;
                }
            }
        }
    }
    props
}

fn apply_jsdoc_props(script: &str, mut props: Vec<PropDef>) -> Vec<PropDef> {
    if let Ok(re) = Regex::new(r"@param\s+\{([^}]+)\}\s+(?:\[)?(\w+)(?:\])?\s+(-?\s*.+)") {
        for cap in re.captures_iter(script) {
            let name = cap[2].to_string();
            let desc = cap[3].trim().to_string();
            let typ = cap[1].trim().to_string();
            if let Some(p) = props.iter_mut().find(|p| p.name == name) {
                p.description = Some(desc);
                if p.r#type.is_none() {
                    p.r#type = Some(typ);
                }
            } else {
                props.push(PropDef {
                    name,
                    r#type: Some(typ),
                    default: None,
                    required: false,
                    description: Some(desc),
                });
            }
        }
    }
    props
}

fn apply_jsdoc_events(script: &str, mut events: Vec<EventDef>) -> Vec<EventDef> {
    if let Ok(re) = Regex::new(r"@emits?\s+(\w+)\s+(-?\s*.+)") {
        for cap in re.captures_iter(script) {
            let name = cap[1].to_string();
            let desc = cap[2].trim().to_string();
            if let Some(e) = events.iter_mut().find(|e| e.name == name) {
                e.description = Some(desc);
            } else {
                events.push(EventDef {
                    name,
                    payload: None,
                    description: Some(desc),
                });
            }
        }
    }
    events
}


fn extract_events(script: &str) -> Vec<EventDef> {
    let mut events = Vec::new();
    let emit_event = |name: String| EventDef {
        name,
        payload: None,
        description: None,
    };

    if let Some(block) = extract_balanced_block(script, "defineEmits<", ">") {
        if let Ok(re) = Regex::new(r#"\(\s*e\s*:\s*['"]([^'"]+)['"]"#) {
            events.extend(capture_all(&re, &block).into_iter().map(emit_event));
        }
        if let Ok(re) = Regex::new(&format!(r#"{QUOTED}\s*:"#)) {
            events.extend(capture_all(&re, &block).into_iter().map(emit_event));
        }
    } else if let Some(block) = extract_balanced_block(script, "defineEmits([", "])") {
        if let Ok(re) = Regex::new(QUOTED) {
            events.extend(capture_all(&re, &block).into_iter().map(emit_event));
        }
    } else if let Some(block) = extract_balanced_block(script, "defineEmits({", "})") {
        if let Ok(re) = Regex::new(QUOTED) {
            events.extend(capture_all(&re, &block).into_iter().map(emit_event));
        }
    }

    if let Some(block) = extract_emits_option(script) {
        if let Ok(re) = Regex::new(QUOTED) {
            events.extend(capture_all(&re, &block).into_iter().map(emit_event));
        }
    }

    dedupe_events(events)
}

fn extract_slots(source: &str) -> Vec<SlotDef> {
    let mut slots = vec![SlotDef {
        name: "default".into(),
        description: Some("Default slot".into()),
    }];

    if let Some(template) = extract_template(source) {
        if let Ok(re) = Regex::new(r#"<slot\s+name\s*=\s*['"]([^'"]+)['"]"#) {
            for name in capture_all(&re, &template) {
                slots.push(SlotDef {
                    name,
                    description: None,
                });
            }
        }
        if let Ok(re) = Regex::new(r#"#(\w+)\s*=\s*"#) {
            for name in capture_all(&re, &template) {
                if name != "default" {
                    slots.push(SlotDef {
                        name,
                        description: Some("scoped slot".into()),
                    });
                }
            }
        }
        // slot props: #header="{ title }"
        if let Ok(re) = Regex::new(r#"#(\w+)\s*=\s*\{([^}]+)\}"#) {
            for cap in re.captures_iter(&template) {
                let name = cap[1].to_string();
                let props_desc = cap[2].trim().to_string();
                if let Some(s) = slots.iter_mut().find(|s| s.name == name) {
                    s.description = Some(format!("scoped: {props_desc}"));
                } else {
                    slots.push(SlotDef {
                        name,
                        description: Some(format!("scoped: {props_desc}")),
                    });
                }
            }
        }
    }

    dedupe_slots(slots)
}

fn extract_template(source: &str) -> Option<String> {
    Regex::new(r"(?s)<template[^>]*>(.*)</template>")
        .ok()?
        .captures(source)
        .map(|c| c[1].to_string())
}

fn extract_balanced_block(source: &str, start: &str, end: &str) -> Option<String> {
    let start_idx = source.find(start)?;
    let content_start = start_idx + start.len();
    let rest = &source[content_start..];
    let end_idx = rest.find(end)?;
    Some(rest[..end_idx].to_string())
}

fn extract_props_option(source: &str) -> Option<String> {
    Regex::new(r"(?s)props\s*:\s*\{([^}]*(?:\{[^}]*\}[^}]*)*)\}")
        .ok()?
        .captures(source)
        .map(|c| c[1].to_string())
}

fn extract_emits_option(source: &str) -> Option<String> {
    Regex::new(r"(?s)emits\s*:\s*\[([^\]]*)\]")
        .ok()?
        .captures(source)
        .map(|c| c[1].to_string())
}

fn parse_ts_interface_props(block: &str) -> Vec<PropDef> {
    let mut props = Vec::new();
    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if let Some(caps) = Regex::new(r"(\w+)\??\s*:\s*([^;]+)")
            .ok()
            .and_then(|re| re.captures(line))
        {
            props.push(PropDef {
                name: caps[1].to_string(),
                r#type: Some(caps[2].trim().trim_end_matches(',').to_string()),
                default: None,
                required: !line.contains('?'),
                description: None,
            });
        }
    }
    props
}

fn parse_object_props_detailed(block: &str) -> Vec<PropDef> {
    let mut props = Vec::new();
    // name: { type: X, default: Y, required: true }
    if let Ok(re) = Regex::new(
        r"(?s)(\w+)\s*:\s*\{([^}]*(?:\{[^}]*\}[^}]*)*)\}",
    ) {
        for cap in re.captures_iter(block) {
            let name = cap[1].to_string();
            if ["type", "default", "required", "validator"].contains(&name.as_str()) {
                continue;
            }
            let inner = &cap[2];
            let prop_type = Regex::new(r"type\s*:\s*([^,\}]+)")
                .ok()
                .and_then(|r| r.captures(inner))
                .map(|c| c[1].trim().to_string());
            let default = Regex::new(r"default\s*:\s*([^,\}]+)")
                .ok()
                .and_then(|r| r.captures(inner))
                .map(|c| c[1].trim().to_string());
            let required = inner.contains("required: true") || inner.contains("required:true");
            props.push(PropDef {
                name,
                r#type: prop_type,
                default,
                required,
                description: None,
            });
        }
    }
    // shorthand: name: String
    if let Ok(re) = Regex::new(r"(\w+)\s*:\s*(String|Number|Boolean|Array|Object|Function)") {
        for cap in re.captures_iter(block) {
            let name = cap[1].to_string();
            if props.iter().any(|p| p.name == name) {
                continue;
            }
            props.push(PropDef {
                name,
                r#type: Some(cap[2].to_string()),
                default: None,
                required: true,
                description: None,
            });
        }
    }
    props
}

fn parse_array_props(block: &str) -> Vec<PropDef> {
    Regex::new(QUOTED)
        .ok()
        .map(|re| {
            capture_all(&re, block)
                .into_iter()
                .map(|name| PropDef {
                    name,
                    r#type: None,
                    default: None,
                    required: true,
                    description: None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn dedupe_props(mut props: Vec<PropDef>) -> Vec<PropDef> {
    props.sort_by(|a, b| a.name.cmp(&b.name));
    props.dedup_by(|a, b| a.name == b.name);
    props
}

fn dedupe_events(mut events: Vec<EventDef>) -> Vec<EventDef> {
    events.sort_by(|a, b| a.name.cmp(&b.name));
    events.dedup_by(|a, b| a.name == b.name);
    events
}

fn dedupe_slots(mut slots: Vec<SlotDef>) -> Vec<SlotDef> {
    slots.sort_by(|a, b| a.name.cmp(&b.name));
    slots.dedup_by(|a, b| a.name == b.name);
    slots
}

fn find_docs(component_path: &Path) -> String {
    let dir = component_path.parent().unwrap_or(component_path);
    let stem = component_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    let candidates = [
        dir.join(format!("{stem}.md")),
        dir.join("README.md"),
        dir.join("index.md"),
        dir.join("doc.md"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                return content;
            }
        }
    }
    String::new()
}

fn find_examples(component_path: &Path, component_name: &str) -> Vec<(String, String)> {
    let dir = component_path.parent().unwrap_or(component_path);
    let mut examples = Vec::new();

    let example_dir = dir.join("examples");
    if example_dir.is_dir() {
        for entry in WalkDir::new(&example_dir).max_depth(2) {
            if let Ok(entry) = entry {
                if entry.file_type().is_file() {
                    let ext = entry
                        .path()
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    if ["vue", "md", "tsx", "jsx"].contains(&ext) {
                        if let Ok(code) = std::fs::read_to_string(entry.path()) {
                            let title = entry
                                .path()
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("example")
                                .to_string();
                            examples.push((title, code));
                        }
                    }
                }
            }
        }
    }

    let demo_path = dir.join(format!("{component_name}.demo.vue"));
    if demo_path.exists() {
        if let Ok(code) = std::fs::read_to_string(&demo_path) {
            examples.push(("demo".into(), code));
        }
    }

    examples
}
