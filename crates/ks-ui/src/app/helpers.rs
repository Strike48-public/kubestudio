use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

/// Format a Kubernetes timestamp as a human-readable age (e.g., "2d", "5h", "3m")
pub fn format_age(timestamp: Option<&Time>) -> String {
    if let Some(ts) = timestamp {
        let created = ts.0;
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(created);

        if duration.num_days() > 0 {
            format!("{}d", duration.num_days())
        } else if duration.num_hours() > 0 {
            format!("{}h", duration.num_hours())
        } else if duration.num_minutes() > 0 {
            format!("{}m", duration.num_minutes())
        } else {
            format!("{}s", duration.num_seconds().max(0))
        }
    } else {
        "-".to_string()
    }
}
