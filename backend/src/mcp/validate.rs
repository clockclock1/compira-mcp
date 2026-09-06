use crate::db::Database;
use regex::Regex;
use std::collections::HashSet;

pub fn validate_code_internal(
    db: &Database,
    code: &str,
    library_id: Option<&str>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut violations = Vec::new();

    let libraries = if let Some(lid) = library_id {
        db.get_library(lid)?
            .map(|l| vec![l])
            .unwrap_or_default()
    } else {
        db.list_libraries()?
    };

    let tag_names = extract_tag_names(code);

    for lib in &libraries {
        if let Some(rules) = &lib.rules {
            for rule in rules.lines() {
                let rule = rule.trim();
                if rule.starts_with("forbid:") {
                    let pattern = rule.trim_start_matches("forbid:").trim();
                    if code.contains(pattern) {
                        violations.push(serde_json::json!({
                            "type": "forbidden_pattern",
                            "library": lib.name,
                            "pattern": pattern,
                            "message": format!("Code contains forbidden pattern: {pattern}"),
                        }));
                    }
                }
                if rule.starts_with("require:") {
                    let pattern = rule.trim_start_matches("require:").trim();
                    if !code.contains(pattern) {
                        violations.push(serde_json::json!({
                            "type": "missing_required",
                            "library": lib.name,
                            "pattern": pattern,
                            "message": format!("Code must contain: {pattern}"),
                        }));
                    }
                }
            }
        }

        if tag_names.is_empty() {
            continue;
        }

        let names: Vec<String> = tag_names.iter().cloned().collect();
        let components = db.find_components_by_names(&lib.id, &names)?;
        for component in components {
            let tag_pattern = format!("<{}", component.name);
            let pascal = to_pascal_case(&component.name);
            let pascal_pattern = format!("<{}", pascal);

            if code.contains(&tag_pattern) || code.contains(&pascal_pattern) {
                for prop in &component.props {
                    if prop.required {
                        let prop_pattern = format!("{}=", prop.name);
                        let bind_pattern = format!(":{}=", prop.name);
                        if !code.contains(&prop_pattern) && !code.contains(&bind_pattern) {
                            violations.push(serde_json::json!({
                                "type": "missing_required_prop",
                                "library": lib.name,
                                "component": component.name,
                                "prop": prop.name,
                                "message": format!(
                                    "Component '{}' requires prop '{}' but it was not found",
                                    component.name, prop.name
                                ),
                            }));
                        }
                    }
                }
            }
        }
    }

    Ok(violations)
}

fn extract_tag_names(code: &str) -> HashSet<String> {
    let re = Regex::new(r"</?([A-Za-z][\w.-]*)").unwrap();
    let mut names = HashSet::new();
    for cap in re.captures_iter(code) {
        if let Some(m) = cap.get(1) {
            let name = m.as_str();
            // skip common HTML tags
            let lower = name.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "div"
                    | "span"
                    | "p"
                    | "a"
                    | "ul"
                    | "li"
                    | "ol"
                    | "table"
                    | "tr"
                    | "td"
                    | "th"
                    | "thead"
                    | "tbody"
                    | "img"
                    | "input"
                    | "button"
                    | "form"
                    | "label"
                    | "select"
                    | "option"
                    | "textarea"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "section"
                    | "header"
                    | "footer"
                    | "main"
                    | "nav"
                    | "template"
                    | "script"
                    | "style"
                    | "svg"
                    | "path"
                    | "i"
                    | "b"
                    | "em"
                    | "strong"
            ) {
                continue;
            }
            names.insert(name.to_string());
            names.insert(to_pascal_case(name));
            names.insert(to_kebab_case(name));
        }
    }
    names
}

fn to_pascal_case(s: &str) -> String {
    s.split(['-', '_', '.'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

fn to_kebab_case(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}
