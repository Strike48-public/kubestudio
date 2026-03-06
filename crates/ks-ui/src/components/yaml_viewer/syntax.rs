//! YAML syntax highlighting

/// Apply syntax highlighting to a YAML line
pub fn highlight_yaml_line(line: &str) -> String {
    let line = line
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");

    if let Some(pos) = line.find('#') {
        let before = &line[..pos];
        let comment = &line[pos..];
        return format!(
            "{}<span class=\"yaml-comment\">{}</span>",
            highlight_yaml_content(before),
            comment
        );
    }

    highlight_yaml_content(&line)
}

/// Highlight YAML content (keys, values, strings, numbers, booleans)
fn highlight_yaml_content(text: &str) -> String {
    if let Some(pos) = text.find(':') {
        let key = &text[..pos];
        let rest = &text[pos..];
        let highlighted_key = format!("<span class=\"yaml-key\">{}</span>", key.trim());
        let value = rest.trim_start_matches(':').trim_start();
        let highlighted_value = if value.is_empty() {
            String::new()
        } else {
            highlight_yaml_value(value)
        };

        format!(
            "{}:{}",
            highlighted_key,
            if highlighted_value.is_empty() {
                String::new()
            } else {
                format!(" {}", highlighted_value)
            }
        )
    } else if text.trim_start().starts_with('-') {
        let indent = &text[..text.len() - text.trim_start().len()];
        let rest = text.trim_start().trim_start_matches('-').trim_start();
        format!(
            "{}<span class=\"yaml-key\">-</span> {}",
            indent,
            highlight_yaml_value(rest)
        )
    } else {
        text.to_string()
    }
}

/// Highlight YAML values (strings, numbers, booleans, null)
fn highlight_yaml_value(value: &str) -> String {
    let trimmed = value.trim();

    if matches!(trimmed, "true" | "false" | "yes" | "no" | "on" | "off") {
        return format!("<span class=\"yaml-boolean\">{}</span>", trimmed);
    }

    if matches!(trimmed, "null" | "~" | "") {
        return format!(
            "<span class=\"yaml-null\">{}</span>",
            if trimmed.is_empty() { "null" } else { trimmed }
        );
    }

    if trimmed.parse::<f64>().is_ok() {
        return format!("<span class=\"yaml-number\">{}</span>", trimmed);
    }

    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        return format!("<span class=\"yaml-string\">{}</span>", trimmed);
    }

    format!("<span class=\"yaml-string\">{}</span>", trimmed)
}
