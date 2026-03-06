//! YAML to kubectl describe format conversion

use chrono::DateTime;
use serde_json::Value as JsonValue;

/// Convert YAML to human-readable kubectl describe format
pub fn yaml_to_describe_format(yaml: &str) -> String {
    let value: JsonValue = match serde_yaml::from_str(yaml) {
        Ok(v) => v,
        Err(e) => return format!("Error parsing YAML: {}", e),
    };

    let mut output = Vec::new();

    let get_str = |v: &JsonValue, path: &str| -> String {
        path.split('.')
            .try_fold(v, |acc, key| acc.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or("<none>")
            .to_string()
    };

    let format_time = |time_str: &str| -> String {
        if let Ok(dt) = DateTime::parse_from_rfc3339(time_str) {
            dt.format("%a, %d %b %Y %H:%M:%S %z").to_string()
        } else {
            time_str.to_string()
        }
    };

    let _kind = get_str(&value, "kind");
    let name = get_str(&value, "metadata.name");
    let namespace = get_str(&value, "metadata.namespace");

    output.push(format!("Name:             {}", name));

    if namespace != "<none>" {
        output.push(format!("Namespace:        {}", namespace));
    }

    if let Some(priority) = value.get("spec").and_then(|v| v.get("priority")) {
        output.push(format!("Priority:         {}", priority));
    }

    if let Some(sa) = value.get("spec").and_then(|v| v.get("serviceAccountName")) {
        output.push(format!(
            "Service Account:  {}",
            sa.as_str().unwrap_or("default")
        ));
    }

    if let Some(node) = value.get("spec").and_then(|v| v.get("nodeName")) {
        let node_str = node.as_str().unwrap_or("");
        if let Some(host_ip) = value
            .get("status")
            .and_then(|v| v.get("hostIP"))
            .and_then(|v| v.as_str())
        {
            output.push(format!("Node:             {}/{}", node_str, host_ip));
        } else {
            output.push(format!("Node:             {}", node_str));
        }
    }

    if let Some(start_time) = value
        .get("status")
        .and_then(|v| v.get("startTime"))
        .and_then(|v| v.as_str())
    {
        output.push(format!("Start Time:       {}", format_time(start_time)));
    }

    // Labels
    format_labels(&value, &mut output);

    // Annotations
    format_annotations(&value, &mut output);

    // Status
    if let Some(phase) = value
        .get("status")
        .and_then(|v| v.get("phase"))
        .and_then(|v| v.as_str())
    {
        output.push(format!("Status:           {}", phase));
    }

    // IP
    if let Some(pod_ip) = value
        .get("status")
        .and_then(|v| v.get("podIP"))
        .and_then(|v| v.as_str())
    {
        output.push(format!("IP:               {}", pod_ip));
    }

    // IPs
    if let Some(ips) = value
        .get("status")
        .and_then(|v| v.get("podIPs"))
        .and_then(|v| v.as_array())
        && !ips.is_empty()
    {
        output.push("IPs:".to_string());
        for ip in ips {
            if let Some(ip_str) = ip.get("ip").and_then(|v| v.as_str()) {
                output.push(format!("  IP:           {}", ip_str));
            }
        }
    }

    // Controlled By
    if let Some(owners) = value
        .get("metadata")
        .and_then(|v| v.get("ownerReferences"))
        .and_then(|v| v.as_array())
        && let Some(owner) = owners.first()
        && let (Some(kind), Some(name)) = (
            owner.get("kind").and_then(|v| v.as_str()),
            owner.get("name").and_then(|v| v.as_str()),
        )
    {
        output.push(format!("Controlled By:  {}/{}", kind, name));
    }

    // Containers
    format_containers(&value, &mut output, &format_time);

    // Conditions
    format_conditions(&value, &mut output);

    // Volumes
    format_volumes(&value, &mut output);

    // QoS Class
    if let Some(qos) = value
        .get("status")
        .and_then(|v| v.get("qosClass"))
        .and_then(|v| v.as_str())
    {
        output.push(format!("QoS Class:                   {}", qos));
    }

    // Node Selectors
    format_node_selectors(&value, &mut output);

    // Tolerations
    format_tolerations(&value, &mut output);

    output.push("Events:                      <none>".to_string());

    output.join("\n")
}

fn format_labels(value: &JsonValue, output: &mut Vec<String>) {
    if let Some(labels) = value
        .get("metadata")
        .and_then(|v| v.get("labels"))
        .and_then(|v| v.as_object())
    {
        if !labels.is_empty() {
            output.push(
                "Labels:           ".to_string()
                    + &labels
                        .iter()
                        .next()
                        .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                        .unwrap_or_default(),
            );
            for (k, v) in labels.iter().skip(1) {
                output.push(format!(
                    "                  {}={}",
                    k,
                    v.as_str().unwrap_or("")
                ));
            }
        }
    } else {
        output.push("Labels:           <none>".to_string());
    }
}

fn format_annotations(value: &JsonValue, output: &mut Vec<String>) {
    if let Some(annotations) = value
        .get("metadata")
        .and_then(|v| v.get("annotations"))
        .and_then(|v| v.as_object())
    {
        if !annotations.is_empty() {
            output.push(
                "Annotations:      ".to_string()
                    + &annotations
                        .iter()
                        .next()
                        .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                        .unwrap_or_default(),
            );
            for (k, v) in annotations.iter().skip(1) {
                output.push(format!(
                    "                  {}={}",
                    k,
                    v.as_str().unwrap_or("")
                ));
            }
        }
    } else {
        output.push("Annotations:      <none>".to_string());
    }
}

fn format_containers<F>(value: &JsonValue, output: &mut Vec<String>, format_time: &F)
where
    F: Fn(&str) -> String,
{
    if let Some(containers) = value
        .get("spec")
        .and_then(|v| v.get("containers"))
        .and_then(|v| v.as_array())
    {
        output.push("Containers:".to_string());
        for container in containers {
            if let Some(name) = container.get("name").and_then(|v| v.as_str()) {
                output.push(format!("  {}:", name));

                format_container_status(value, name, output, format_time);
                format_container_image(container, value, name, output);
                format_container_ports(container, output);
                format_container_state(value, name, output, format_time);
                format_container_resources(container, output);
                format_container_probes(container, output);
                format_container_env(container, output);
                format_container_mounts(container, output);
            }
        }
    }
}

fn format_container_status<F>(
    value: &JsonValue,
    name: &str,
    output: &mut Vec<String>,
    _format_time: &F,
) where
    F: Fn(&str) -> String,
{
    if let Some(container_statuses) = value
        .get("status")
        .and_then(|v| v.get("containerStatuses"))
        .and_then(|v| v.as_array())
    {
        for status in container_statuses {
            if status.get("name").and_then(|v| v.as_str()) == Some(name) {
                if let Some(cid) = status.get("containerID").and_then(|v| v.as_str()) {
                    output.push(format!("    Container ID:   {}", cid));
                }
                break;
            }
        }
    }
}

fn format_container_image(
    container: &JsonValue,
    value: &JsonValue,
    name: &str,
    output: &mut Vec<String>,
) {
    if let Some(image) = container.get("image").and_then(|v| v.as_str()) {
        output.push(format!("    Image:          {}", image));
    }

    if let Some(container_statuses) = value
        .get("status")
        .and_then(|v| v.get("containerStatuses"))
        .and_then(|v| v.as_array())
    {
        for status in container_statuses {
            if status.get("name").and_then(|v| v.as_str()) == Some(name) {
                if let Some(image_id) = status.get("imageID").and_then(|v| v.as_str()) {
                    output.push(format!("    Image ID:       {}", image_id));
                }
                break;
            }
        }
    }
}

fn format_container_ports(container: &JsonValue, output: &mut Vec<String>) {
    if let Some(ports) = container.get("ports").and_then(|v| v.as_array()) {
        for port in ports {
            if let Some(container_port) = port.get("containerPort") {
                let protocol = port
                    .get("protocol")
                    .and_then(|v| v.as_str())
                    .unwrap_or("TCP");
                let port_name = port.get("name").and_then(|v| v.as_str()).unwrap_or("");
                output.push(format!(
                    "    Port:           {}/{} ({})",
                    container_port, protocol, port_name
                ));
            }
        }
    }
}

fn format_container_state<F>(
    value: &JsonValue,
    name: &str,
    output: &mut Vec<String>,
    format_time: &F,
) where
    F: Fn(&str) -> String,
{
    if let Some(container_statuses) = value
        .get("status")
        .and_then(|v| v.get("containerStatuses"))
        .and_then(|v| v.as_array())
    {
        for status in container_statuses {
            if status.get("name").and_then(|v| v.as_str()) == Some(name) {
                if let Some(state) = status.get("state").and_then(|v| v.as_object())
                    && let Some(running) = state.get("running")
                {
                    output.push("    State:          Running".to_string());
                    if let Some(started) = running.get("startedAt").and_then(|v| v.as_str()) {
                        output.push(format!("      Started:      {}", format_time(started)));
                    }
                }
                if let Some(ready) = status.get("ready").and_then(|v| v.as_bool()) {
                    output.push(format!(
                        "    Ready:          {}",
                        if ready { "True" } else { "False" }
                    ));
                }
                if let Some(restart_count) = status.get("restartCount") {
                    output.push(format!("    Restart Count:  {}", restart_count));
                }
                break;
            }
        }
    }
}

fn format_container_resources(container: &JsonValue, output: &mut Vec<String>) {
    if let Some(resources) = container.get("resources").and_then(|v| v.as_object()) {
        if let Some(limits) = resources.get("limits").and_then(|v| v.as_object()) {
            output.push("    Limits:".to_string());
            for (k, v) in limits {
                output.push(format!("      {}:     {}", k, v.as_str().unwrap_or("")));
            }
        }
        if let Some(requests) = resources.get("requests").and_then(|v| v.as_object()) {
            output.push("    Requests:".to_string());
            for (k, v) in requests {
                output.push(format!("      {}:      {}", k, v.as_str().unwrap_or("")));
            }
        }
    }
}

fn format_container_probes(container: &JsonValue, output: &mut Vec<String>) {
    if let Some(liveness) = container.get("livenessProbe") {
        let probe_str = format_probe("Liveness", liveness);
        output.push(probe_str);
    }

    if let Some(readiness) = container.get("readinessProbe") {
        let probe_str = format_probe("Readiness", readiness);
        output.push(probe_str);
    }
}

fn format_probe(name: &str, probe: &JsonValue) -> String {
    let mut probe_str = format!("    {}:   ", name);
    if let Some(tcp) = probe.get("tcpSocket") {
        if let Some(port) = tcp.get("port") {
            probe_str.push_str(&format!("tcp-socket :{}", port));
        }
    } else if let Some(http) = probe.get("httpGet") {
        if let Some(path) = http.get("path").and_then(|v| v.as_str()) {
            probe_str.push_str(&format!("http-get {}", path));
        }
    } else if probe.get("exec").is_some() {
        probe_str.push_str("exec");
    }
    if let Some(delay) = probe.get("initialDelaySeconds") {
        probe_str.push_str(&format!(" delay={}s", delay));
    }
    if let Some(timeout) = probe.get("timeoutSeconds") {
        probe_str.push_str(&format!(" timeout={}s", timeout));
    }
    if let Some(period) = probe.get("periodSeconds") {
        probe_str.push_str(&format!(" period={}s", period));
    }
    if let Some(success) = probe.get("successThreshold") {
        probe_str.push_str(&format!(" #success={}", success));
    }
    if let Some(failure) = probe.get("failureThreshold") {
        probe_str.push_str(&format!(" #failure={}", failure));
    }
    probe_str
}

fn format_container_env(container: &JsonValue, output: &mut Vec<String>) {
    if let Some(env) = container.get("env").and_then(|v| v.as_array())
        && !env.is_empty()
    {
        output.push("    Environment:".to_string());
        for var in env {
            if let Some(var_name) = var.get("name").and_then(|v| v.as_str()) {
                let var_value = var.get("value").and_then(|v| v.as_str()).unwrap_or("<set>");
                output.push(format!("      {}:  {}", var_name, var_value));
            }
        }
    }
}

fn format_container_mounts(container: &JsonValue, output: &mut Vec<String>) {
    if let Some(mounts) = container.get("volumeMounts").and_then(|v| v.as_array())
        && !mounts.is_empty()
    {
        output.push("    Mounts:".to_string());
        for mount in mounts {
            if let (Some(mount_path), Some(mount_name)) = (
                mount.get("mountPath").and_then(|v| v.as_str()),
                mount.get("name").and_then(|v| v.as_str()),
            ) {
                let ro = mount
                    .get("readOnly")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                output.push(format!(
                    "      {} from {} ({})",
                    mount_path,
                    mount_name,
                    if ro { "ro" } else { "rw" }
                ));
            }
        }
    }
}

fn format_conditions(value: &JsonValue, output: &mut Vec<String>) {
    if let Some(conditions) = value
        .get("status")
        .and_then(|v| v.get("conditions"))
        .and_then(|v| v.as_array())
        && !conditions.is_empty()
    {
        output.push("Conditions:".to_string());
        output.push("  Type                        Status".to_string());
        for condition in conditions {
            if let (Some(type_), Some(status)) = (
                condition.get("type").and_then(|v| v.as_str()),
                condition.get("status").and_then(|v| v.as_str()),
            ) {
                output.push(format!("  {:<27} {}", type_, status));
            }
        }
    }
}

fn format_volumes(value: &JsonValue, output: &mut Vec<String>) {
    if let Some(volumes) = value
        .get("spec")
        .and_then(|v| v.get("volumes"))
        .and_then(|v| v.as_array())
        && !volumes.is_empty()
    {
        output.push("Volumes:".to_string());
        for volume in volumes {
            if let Some(vol_name) = volume.get("name").and_then(|v| v.as_str()) {
                output.push(format!("  {}:", vol_name));
                if let Some(pvc) = volume.get("persistentVolumeClaim") {
                    output.push("    Type:                    PersistentVolumeClaim".to_string());
                    if let Some(claim_name) = pvc.get("claimName").and_then(|v| v.as_str()) {
                        output.push(format!("    ClaimName:               {}", claim_name));
                    }
                } else if let Some(config_map) = volume.get("configMap") {
                    output.push("    Type:        ConfigMap".to_string());
                    if let Some(cm_name) = config_map.get("name").and_then(|v| v.as_str()) {
                        output.push(format!("    Name:        {}", cm_name));
                    }
                } else if volume.get("projected").is_some() {
                    output.push("    Type:                    Projected (a volume that contains injected data from multiple sources)".to_string());
                } else if volume.get("secret").is_some() {
                    output.push("    Type:        Secret".to_string());
                }
            }
        }
    }
}

fn format_node_selectors(value: &JsonValue, output: &mut Vec<String>) {
    if let Some(node_selector) = value
        .get("spec")
        .and_then(|v| v.get("nodeSelector"))
        .and_then(|v| v.as_object())
    {
        if !node_selector.is_empty() {
            output.push(
                "Node-Selectors:              ".to_string()
                    + &node_selector
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                        .collect::<Vec<_>>()
                        .join(","),
            );
        }
    } else {
        output.push("Node-Selectors:              <none>".to_string());
    }
}

fn format_tolerations(value: &JsonValue, output: &mut Vec<String>) {
    if let Some(tolerations) = value
        .get("spec")
        .and_then(|v| v.get("tolerations"))
        .and_then(|v| v.as_array())
        && !tolerations.is_empty()
    {
        output.push(
            "Tolerations:                 ".to_string()
                + &{
                    let first = &tolerations[0];
                    let key = first.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    let effect = first.get("effect").and_then(|v| v.as_str()).unwrap_or("");
                    let op = first
                        .get("operator")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Exists");
                    let seconds = first
                        .get("tolerationSeconds")
                        .and_then(|v| v.as_i64())
                        .map(|s| format!(" for {}s", s))
                        .unwrap_or_default();
                    format!("{}:{} op={}{}", key, effect, op, seconds)
                },
        );
        for toleration in tolerations.iter().skip(1) {
            let key = toleration.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let effect = toleration
                .get("effect")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let op = toleration
                .get("operator")
                .and_then(|v| v.as_str())
                .unwrap_or("Exists");
            let seconds = toleration
                .get("tolerationSeconds")
                .and_then(|v| v.as_i64())
                .map(|s| format!(" for {}s", s))
                .unwrap_or_default();
            output.push(format!(
                "                             {}:{} op={}{}",
                key, effect, op, seconds
            ));
        }
    }
}
