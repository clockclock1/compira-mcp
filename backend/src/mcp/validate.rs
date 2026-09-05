use crate::db::Database;

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

        let components = db.list_components_by_library(&lib.id)?;
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

fn to_pascal_case(s: &str) -> String {
    s.split('-')
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}
