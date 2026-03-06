//! YAML helper functions

/// Strip managedFields from YAML content
pub fn strip_managed_fields(yaml: &str) -> String {
    let lines: Vec<&str> = yaml.lines().collect();
    let mut result = Vec::new();
    let mut skip_depth = None::<usize>;
    let mut in_managed_fields = false;

    for line in lines {
        if in_managed_fields && line.trim().is_empty() {
            continue;
        }

        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();

        if trimmed.starts_with("managedFields:") {
            in_managed_fields = true;
            skip_depth = Some(indent);
            continue;
        }

        if in_managed_fields {
            if let Some(depth) = skip_depth {
                let is_list_item = trimmed.starts_with("- ");
                let should_exit = indent < depth || (indent == depth && !is_list_item);

                if should_exit {
                    in_managed_fields = false;
                    skip_depth = None;
                    result.push(line);
                } else {
                    continue;
                }
            }
        } else {
            result.push(line);
        }
    }

    result.join("\n")
}
