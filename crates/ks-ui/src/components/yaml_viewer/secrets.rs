//! Secret masking logic for YAML content

/// Mask secret data values in YAML content
/// Replaces values under `data:` and `stringData:` sections with masked placeholder
/// Also masks secret values in kubectl.kubernetes.io/last-applied-configuration annotations
pub fn mask_secret_data(yaml: &str) -> String {
    let lines: Vec<&str> = yaml.lines().collect();
    let mut result = Vec::new();
    let mut in_data_section = false;
    let mut data_indent = 0usize;

    for line in lines {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();

        // Check if we're entering a data or stringData section
        if trimmed.starts_with("data:") || trimmed.starts_with("stringData:") {
            in_data_section = true;
            data_indent = indent;
            result.push(line.to_string());
            continue;
        }

        // Check if we're exiting the data section
        if in_data_section
            && !trimmed.is_empty()
            && indent <= data_indent
            && !trimmed.starts_with('-')
        {
            in_data_section = false;
        }

        if in_data_section && !trimmed.is_empty() {
            // This is a line inside data section
            // Check if it's a key: value line (not a continuation or list item)
            if let Some(colon_pos) = trimmed.find(':') {
                let after_colon = &trimmed[colon_pos + 1..];
                // If there's a value after the colon, mask it
                if !after_colon.trim().is_empty()
                    && !after_colon.trim().starts_with('|')
                    && !after_colon.trim().starts_with('>')
                {
                    let key = &trimmed[..colon_pos + 1];
                    let masked = format!("{}{} ••••••••", " ".repeat(indent), key);
                    result.push(masked);
                    continue;
                }
            }
        }

        // Check for last-applied-configuration annotation containing secret data
        // This appears as JSON with "data":{...} or "stringData":{...}
        if line.contains("last-applied-configuration")
            || (line.contains(r#""data":"#) && line.contains(r#""kind":"Secret""#))
            || (line.contains(r#""stringData":"#) && line.contains(r#""kind":"Secret""#))
        {
            let masked_line = mask_json_secret_data(line);
            result.push(masked_line);
            continue;
        }

        result.push(line.to_string());
    }

    result.join("\n")
}

/// Mask secret data values within JSON content (for last-applied-configuration annotations)
fn mask_json_secret_data(line: &str) -> String {
    let mut result = line.to_string();

    // Find and mask "data":{...} patterns
    // Pattern: "data":{"key":"value","key2":"value2"}
    if let Some(data_start) = result.find(r#""data":{"#) {
        let content_start = data_start + 8; // Position after "data":{"
        // Start searching from inside the braces (after the opening {)
        if let Some(data_end) = find_matching_brace(&result[content_start..]) {
            let data_end_pos = content_start + data_end; // Position of closing }
            let data_content = &result[content_start..data_end_pos];
            let masked_content = mask_json_values(data_content);
            result = format!(
                "{}\"data\":{{{}}}{}",
                &result[..data_start],
                masked_content,
                &result[data_end_pos + 1..]
            );
        }
    }

    // Also handle "stringData":{...} patterns
    if let Some(data_start) = result.find(r#""stringData":{"#) {
        let content_start = data_start + 14; // Position after "stringData":{"
        if let Some(data_end) = find_matching_brace(&result[content_start..]) {
            let data_end_pos = content_start + data_end;
            let data_content = &result[content_start..data_end_pos];
            let masked_content = mask_json_values(data_content);
            result = format!(
                "{}\"stringData\":{{{}}}{}",
                &result[..data_start],
                masked_content,
                &result[data_end_pos + 1..]
            );
        }
    }

    result
}

/// Find the position of the matching closing brace
fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 1;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Mask values in JSON key-value pairs, preserving keys
fn mask_json_values(json_content: &str) -> String {
    let mut result = String::new();
    let mut chars = json_content.chars().peekable();
    let mut in_key = false;
    let mut in_value = false;
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            if in_key || !in_value {
                result.push(c);
            }
            escape_next = false;
            continue;
        }

        match c {
            '\\' => {
                escape_next = true;
                if in_key || !in_value {
                    result.push(c);
                }
            }
            '"' => {
                if !in_string {
                    in_string = true;
                    if !in_value {
                        in_key = true;
                    }
                    result.push(c);
                } else {
                    in_string = false;
                    if in_key {
                        in_key = false;
                        result.push(c);
                    } else if in_value {
                        result.push_str("••••••••\"");
                        in_value = false;
                    } else {
                        result.push(c);
                    }
                }
            }
            ':' if !in_string => {
                result.push(c);
                in_value = true;
                // Skip whitespace after colon
                while chars.peek() == Some(&' ') {
                    chars.next();
                }
                // Check if next char is a quote (string value)
                if chars.peek() == Some(&'"') {
                    result.push('"');
                    chars.next(); // consume the opening quote
                    in_string = true;
                }
            }
            ',' if !in_string => {
                in_value = false;
                result.push(c);
            }
            _ => {
                if in_key || !in_value || !in_string {
                    result.push(c);
                }
                // Skip characters inside value strings (they'll be replaced with mask)
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_secret_data_simple() {
        let yaml = r#"apiVersion: v1
kind: Secret
data:
  DB_URL: c2VjcmV0dmFsdWU=
  PASSWORD: cGFzc3dvcmQ=
metadata:
  name: my-secret
"#;
        let masked = mask_secret_data(yaml);
        assert!(masked.contains("DB_URL: ••••••••"));
        assert!(masked.contains("PASSWORD: ••••••••"));
        assert!(masked.contains("name: my-secret")); // Non-secret data unchanged
    }

    #[test]
    fn test_mask_json_secret_data_only_data_field() {
        let json = r#"{"apiVersion":"v1","data":{"DB_URL":"secret123"},"kind":"Secret","metadata":{"name":"test"}}"#;
        let masked = mask_json_secret_data(json);

        // Should mask only the value inside data
        assert!(masked.contains(r#""data":{"DB_URL":"••••••••"}"#));
        // Should NOT mask other fields
        assert!(masked.contains(r#""apiVersion":"v1""#));
        assert!(masked.contains(r#""kind":"Secret""#));
        assert!(masked.contains(r#""name":"test""#));
    }

    #[test]
    fn test_mask_json_secret_data_multiple_keys() {
        let json = r#"{"data":{"KEY1":"val1","KEY2":"val2"},"kind":"Secret"}"#;
        let masked = mask_json_secret_data(json);

        assert!(masked.contains(r#""KEY1":"••••••••""#));
        assert!(masked.contains(r#""KEY2":"••••••••""#));
        assert!(masked.contains(r#""kind":"Secret""#));
    }

    #[test]
    fn test_mask_secret_data_with_annotation() {
        let yaml = r#"apiVersion: v1
kind: Secret
data:
  DB_URL: c2VjcmV0dmFsdWU=
metadata:
  annotations:
    kubectl.kubernetes.io/last-applied-configuration: |
      {"apiVersion":"v1","data":{"DB_URL":"c2VjcmV0dmFsdWU="},"kind":"Secret","metadata":{"name":"test"}}
  name: test
"#;
        let masked = mask_secret_data(yaml);

        // YAML data section should be masked
        assert!(masked.contains("DB_URL: ••••••••"));
        // JSON annotation should also be masked
        assert!(masked.contains(r#""DB_URL":"••••••••""#));
        // But other JSON fields should not be masked
        assert!(masked.contains(r#""apiVersion":"v1""#));
        assert!(masked.contains(r#""kind":"Secret""#));
    }

    #[test]
    fn test_mask_describe_format_annotation() {
        // Describe format puts annotation on same line as key
        let describe_line = r#"                  kubectl.kubernetes.io/last-applied-configuration={"apiVersion":"v1","data":{"DB_URL":"secret"},"kind":"Secret","metadata":{"name":"test"}}"#;
        let masked = mask_secret_data(describe_line);

        // Secret data should be masked
        assert!(masked.contains(r#""DB_URL":"••••••••""#));
        // Other fields should NOT be masked
        assert!(masked.contains(r#""apiVersion":"v1""#));
        assert!(masked.contains(r#""kind":"Secret""#));
        assert!(masked.contains(r#""name":"test""#));
    }
}
