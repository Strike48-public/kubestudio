use super::helpers::{age_seconds, format_age};
use crate::components::ResourceItem;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{
    ConfigMap, Endpoints, Event, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret,
    Service,
};
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding};
use k8s_openapi::api::storage::v1::StorageClass;

pub fn pods_to_items(pods: &[Pod]) -> Vec<ResourceItem> {
    pods.iter()
        .map(|pod| {
            let metadata = &pod.metadata;
            let status = pod.status.as_ref();

            let ready = status.and_then(|s| {
                s.container_statuses.as_ref().map(|containers| {
                    let ready_count = containers.iter().filter(|c| c.ready).count();
                    let total_count = containers.len();
                    format!("{}/{}", ready_count, total_count)
                })
            });

            let restarts: Option<u32> = status.and_then(|s| {
                s.container_statuses
                    .as_ref()
                    .map(|containers| containers.iter().map(|c| c.restart_count as u32).sum())
            });

            let pod_status = if let Some(s) = status {
                if let Some(containers) = &s.container_statuses {
                    if let Some(waiting_reason) = containers.iter().find_map(|c| {
                        c.state
                            .as_ref()
                            .and_then(|state| state.waiting.as_ref().and_then(|w| w.reason.clone()))
                    }) {
                        waiting_reason
                    } else if let Some(terminated_reason) = containers.iter().find_map(|c| {
                        c.state.as_ref().and_then(|state| {
                            state.terminated.as_ref().and_then(|t| t.reason.clone())
                        })
                    }) {
                        terminated_reason
                    } else if containers.iter().any(|c| !c.ready) {
                        "NotReady".to_string()
                    } else {
                        s.phase.clone().unwrap_or_else(|| "Unknown".to_string())
                    }
                } else {
                    s.phase.clone().unwrap_or_else(|| "Unknown".to_string())
                }
            } else {
                "Unknown".to_string()
            };

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: pod_status,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready,
                restarts,
            }
        })
        .collect()
}

pub fn deployments_to_items(deployments: &[Deployment]) -> Vec<ResourceItem> {
    deployments
        .iter()
        .map(|deployment| {
            let metadata = &deployment.metadata;
            let status = deployment.status.as_ref();

            let ready = status.map(|s| {
                let ready_replicas = s.ready_replicas.unwrap_or(0);
                let replicas = s.replicas.unwrap_or(0);
                format!("{}/{}", ready_replicas, replicas)
            });

            let deployment_status = if let Some(s) = status {
                let ready_replicas = s.ready_replicas.unwrap_or(0);
                let desired_replicas = s.replicas.unwrap_or(0);

                if let Some(conditions) = &s.conditions {
                    if conditions
                        .iter()
                        .any(|c| c.type_ == "ReplicaFailure" && c.status == "True")
                    {
                        "Failed".to_string()
                    } else if conditions
                        .iter()
                        .any(|c| c.type_ == "Progressing" && c.status == "False")
                    {
                        "Stalled".to_string()
                    } else if ready_replicas < desired_replicas {
                        "Progressing".to_string()
                    } else if ready_replicas == desired_replicas && desired_replicas > 0 {
                        "Available".to_string()
                    } else {
                        "Unknown".to_string()
                    }
                } else if ready_replicas < desired_replicas {
                    "Progressing".to_string()
                } else if ready_replicas == desired_replicas && desired_replicas > 0 {
                    "Available".to_string()
                } else {
                    "Unknown".to_string()
                }
            } else {
                "Unknown".to_string()
            };

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: deployment_status,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready,
                restarts: None,
            }
        })
        .collect()
}

pub fn statefulsets_to_items(statefulsets: &[StatefulSet]) -> Vec<ResourceItem> {
    statefulsets
        .iter()
        .map(|sts| {
            let metadata = &sts.metadata;
            let spec = sts.spec.as_ref();
            let status = sts.status.as_ref();

            let ready = status.map(|s| {
                let ready_replicas = s.ready_replicas.unwrap_or(0);
                let replicas = spec.and_then(|sp| sp.replicas).unwrap_or(1);
                format!("{}/{}", ready_replicas, replicas)
            });

            let sts_status = if let Some(ref ready_str) = ready {
                let parts: Vec<&str> = ready_str.split('/').collect::<Vec<&str>>();
                if parts.len() == 2 && parts[0] == parts[1] {
                    "Running".to_string()
                } else {
                    "Pending".to_string()
                }
            } else {
                "Unknown".to_string()
            };

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: sts_status,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready,
                restarts: None,
            }
        })
        .collect()
}

pub fn daemonsets_to_items(daemonsets: &[DaemonSet]) -> Vec<ResourceItem> {
    daemonsets
        .iter()
        .map(|ds| {
            let metadata = &ds.metadata;
            let status = ds.status.as_ref();

            let ready = status.map(|s| {
                let number_ready = s.number_ready;
                let desired = s.desired_number_scheduled;
                format!("{}/{}", number_ready, desired)
            });

            let ds_status = if let Some(s) = status {
                if s.number_ready == s.desired_number_scheduled {
                    "Running".to_string()
                } else if s.number_ready > 0 {
                    "Degraded".to_string()
                } else {
                    "Pending".to_string()
                }
            } else {
                "Unknown".to_string()
            };

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: ds_status,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready,
                restarts: None,
            }
        })
        .collect()
}

pub fn jobs_to_items(jobs: &[Job]) -> Vec<ResourceItem> {
    jobs.iter()
        .map(|job| {
            let metadata = &job.metadata;
            let spec = job.spec.as_ref();
            let status = job.status.as_ref();

            let ready = status.map(|s| {
                let succeeded = s.succeeded.unwrap_or(0);
                let completions = spec.and_then(|sp| sp.completions).unwrap_or(1);
                format!("{}/{}", succeeded, completions)
            });

            let job_status = if let Some(s) = status {
                if s.succeeded.unwrap_or(0) > 0 && s.succeeded == spec.and_then(|sp| sp.completions)
                {
                    "Complete".to_string()
                } else if s.failed.unwrap_or(0) > 0 {
                    "Failed".to_string()
                } else if s.active.unwrap_or(0) > 0 {
                    "Running".to_string()
                } else {
                    "Pending".to_string()
                }
            } else {
                "Unknown".to_string()
            };

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: job_status,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready,
                restarts: None,
            }
        })
        .collect()
}

pub fn cronjobs_to_items(cronjobs: &[CronJob]) -> Vec<ResourceItem> {
    cronjobs
        .iter()
        .map(|cj| {
            let metadata = &cj.metadata;
            let spec = cj.spec.as_ref();
            let status = cj.status.as_ref();

            let schedule = spec.map(|s| s.schedule.clone()).unwrap_or_default();

            let ready = status.and_then(|s| {
                s.last_schedule_time
                    .as_ref()
                    .map(|t| format!("Last: {}", t.0.format("%Y-%m-%d %H:%M")))
            });

            let cj_status = if spec.map(|s| s.suspend.unwrap_or(false)).unwrap_or(false) {
                "Suspended".to_string()
            } else if status
                .and_then(|s| s.active.as_ref())
                .map(|a| !a.is_empty())
                .unwrap_or(false)
            {
                "Active".to_string()
            } else {
                "Ready".to_string()
            };

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: cj_status,
                age: schedule,
                age_seconds: None,
                ready,
                restarts: None,
            }
        })
        .collect()
}

pub fn configmaps_to_items(configmaps: &[ConfigMap]) -> Vec<ResourceItem> {
    configmaps
        .iter()
        .map(|cm| {
            let metadata = &cm.metadata;

            let data_count = cm.data.as_ref().map(|d| d.len()).unwrap_or(0);
            let binary_count = cm.binary_data.as_ref().map(|d| d.len()).unwrap_or(0);
            let total_keys = data_count + binary_count;

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: "Active".to_string(),
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: Some(format!("{} keys", total_keys)),
                restarts: None,
            }
        })
        .collect()
}

pub fn secrets_to_items(secrets: &[Secret], pods: &[Pod]) -> Vec<ResourceItem> {
    secrets
        .iter()
        .map(|secret| {
            let metadata = &secret.metadata;
            let secret_name = metadata.name.clone().unwrap_or_default();
            let secret_namespace = metadata.namespace.clone();

            let in_use = pods.iter().any(|pod| {
                if pod.metadata.namespace != secret_namespace {
                    return false;
                }

                let is_running = pod
                    .status
                    .as_ref()
                    .and_then(|s| s.phase.as_ref())
                    .map(|phase| phase == "Running")
                    .unwrap_or(false);

                if !is_running {
                    return false;
                }

                let spec = match &pod.spec {
                    Some(s) => s,
                    None => return false,
                };

                let used_in_image_pull = spec
                    .image_pull_secrets
                    .as_ref()
                    .is_some_and(|secrets| secrets.iter().any(|s| s.name == secret_name));

                let used_in_volumes = spec.volumes.as_ref().is_some_and(|volumes| {
                    volumes.iter().any(|vol| {
                        vol.secret.as_ref().is_some_and(|secret_ref| {
                            secret_ref.secret_name.as_ref() == Some(&secret_name)
                        })
                    })
                });

                let check_container = |container: &k8s_openapi::api::core::v1::Container| {
                    let env_from = container.env_from.as_ref().is_some_and(|env_from_list| {
                        env_from_list.iter().any(|env_from| {
                            if let Some(secret_ref) = &env_from.secret_ref {
                                return secret_ref.name == secret_name;
                            }
                            false
                        })
                    });

                    let env_vars = container.env.as_ref().is_some_and(|env_list| {
                        env_list.iter().any(|env_var| {
                            if let Some(value_from) = &env_var.value_from
                                && let Some(secret_key) = &value_from.secret_key_ref
                            {
                                return secret_key.name == secret_name;
                            }
                            false
                        })
                    });

                    env_from || env_vars
                };

                let used_in_containers = spec.containers.iter().any(check_container);

                let used_in_init_containers = spec
                    .init_containers
                    .as_ref()
                    .is_some_and(|init_containers| init_containers.iter().any(check_container));

                used_in_image_pull
                    || used_in_volumes
                    || used_in_containers
                    || used_in_init_containers
            });

            let data_count = secret.data.as_ref().map(|d| d.len()).unwrap_or(0);
            let ready = if data_count > 0 {
                Some(format!(
                    "{} key{}",
                    data_count,
                    if data_count == 1 { "" } else { "s" }
                ))
            } else {
                None
            };

            let status = if in_use {
                "In Use".to_string()
            } else {
                "Unused".to_string()
            };

            ResourceItem {
                name: secret_name,
                namespace: secret_namespace,
                status,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready,
                restarts: None,
            }
        })
        .collect()
}

pub fn services_to_items(services: &[Service], endpoints: &[Endpoints]) -> Vec<ResourceItem> {
    // Build a lookup of endpoint address counts by name+namespace
    let endpoint_counts: std::collections::HashMap<(String, Option<String>), usize> = endpoints
        .iter()
        .map(|ep| {
            let name = ep.metadata.name.clone().unwrap_or_default();
            let ns = ep.metadata.namespace.clone();
            let count = ep
                .subsets
                .as_ref()
                .map(|subsets| {
                    subsets
                        .iter()
                        .map(|s| s.addresses.as_ref().map(|a| a.len()).unwrap_or(0))
                        .sum::<usize>()
                })
                .unwrap_or(0);
            ((name, ns), count)
        })
        .collect();

    services
        .iter()
        .map(|svc| {
            let metadata = &svc.metadata;
            let spec = svc.spec.as_ref();

            let service_type = spec
                .and_then(|s| s.type_.clone())
                .unwrap_or_else(|| "ClusterIP".to_string());

            let port_count = spec
                .and_then(|s| s.ports.as_ref().map(|p| p.len()))
                .unwrap_or(0);

            let has_selector = spec
                .and_then(|s| s.selector.as_ref())
                .map(|s| !s.is_empty())
                .unwrap_or(false);

            let ep_count = endpoint_counts
                .get(&(
                    metadata.name.clone().unwrap_or_default(),
                    metadata.namespace.clone(),
                ))
                .copied()
                .unwrap_or(0);

            let svc_status = if service_type == "ExternalName" || !has_selector {
                // ExternalName or no-selector services — no endpoints expected
                "Active".to_string()
            } else if ep_count > 0 {
                "Active".to_string()
            } else {
                "Pending".to_string()
            };

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: svc_status,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: Some(format!(
                    "{} ({} port{})",
                    service_type,
                    port_count,
                    if port_count == 1 { "" } else { "s" }
                )),
                restarts: None,
            }
        })
        .collect()
}

pub fn endpoints_to_items(endpoints: &[Endpoints]) -> Vec<ResourceItem> {
    endpoints
        .iter()
        .map(|ep| {
            let metadata = &ep.metadata;

            let address_count = ep
                .subsets
                .as_ref()
                .map(|subsets| {
                    subsets
                        .iter()
                        .map(|subset| subset.addresses.as_ref().map(|a| a.len()).unwrap_or(0))
                        .sum::<usize>()
                })
                .unwrap_or(0);

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: if address_count > 0 {
                    "Ready".to_string()
                } else {
                    "NotReady".to_string()
                },
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: Some(format!(
                    "{} address{}",
                    address_count,
                    if address_count == 1 { "" } else { "es" }
                )),
                restarts: None,
            }
        })
        .collect()
}

pub fn persistentvolumes_to_items(pvs: &[PersistentVolume]) -> Vec<ResourceItem> {
    pvs.iter()
        .map(|pv| {
            let metadata = &pv.metadata;
            let spec = pv.spec.as_ref();
            let status = pv.status.as_ref();

            let capacity = spec
                .and_then(|s| s.capacity.as_ref())
                .and_then(|cap| cap.get("storage"))
                .map(|storage| storage.0.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            let phase = status
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            let storage_class = spec
                .and_then(|s| s.storage_class_name.clone())
                .unwrap_or_else(|| "None".to_string());

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: None,
                status: phase,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: Some(format!("{} ({})", capacity, storage_class)),
                restarts: None,
            }
        })
        .collect()
}

pub fn persistentvolumeclaims_to_items(pvcs: &[PersistentVolumeClaim]) -> Vec<ResourceItem> {
    pvcs.iter()
        .map(|pvc| {
            let metadata = &pvc.metadata;
            let spec = pvc.spec.as_ref();
            let status = pvc.status.as_ref();

            let phase = status
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            let capacity = status
                .and_then(|s| s.capacity.as_ref())
                .and_then(|cap| cap.get("storage"))
                .map(|storage| storage.0.clone())
                .unwrap_or_else(|| "Pending".to_string());

            let storage_class = spec
                .and_then(|s| s.storage_class_name.clone())
                .unwrap_or_else(|| "default".to_string());

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: phase,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: Some(format!("{} ({})", capacity, storage_class)),
                restarts: None,
            }
        })
        .collect()
}

pub fn ingresses_to_items(ingresses: &[Ingress]) -> Vec<ResourceItem> {
    ingresses
        .iter()
        .map(|ingress| {
            let metadata = &ingress.metadata;
            let spec = ingress.spec.as_ref();
            let status = ingress.status.as_ref();

            let ingress_class = spec
                .and_then(|s| s.ingress_class_name.clone())
                .unwrap_or_else(|| "default".to_string());

            let rule_count = spec
                .and_then(|s| s.rules.as_ref().map(|r| r.len()))
                .unwrap_or(0);

            let lb_info = status
                .and_then(|s| s.load_balancer.as_ref())
                .and_then(|lb| lb.ingress.as_ref())
                .and_then(|ing_list| {
                    if ing_list.is_empty() {
                        None
                    } else {
                        Some(format!(
                            "{} endpoint{}",
                            ing_list.len(),
                            if ing_list.len() == 1 { "" } else { "s" }
                        ))
                    }
                });

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: ingress_class,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: lb_info.or(Some(format!(
                    "{} rule{}",
                    rule_count,
                    if rule_count == 1 { "" } else { "s" }
                ))),
                restarts: None,
            }
        })
        .collect()
}

pub fn storageclasses_to_items(storageclasses: &[StorageClass]) -> Vec<ResourceItem> {
    storageclasses
        .iter()
        .map(|sc| {
            let metadata = &sc.metadata;

            let provisioner = sc.provisioner.clone();

            let reclaim_policy = sc
                .reclaim_policy
                .clone()
                .unwrap_or_else(|| "Delete".to_string());

            let binding_mode = sc
                .volume_binding_mode
                .clone()
                .unwrap_or_else(|| "Immediate".to_string());

            let is_default = metadata
                .annotations
                .as_ref()
                .and_then(|annotations| {
                    annotations
                        .get("storageclass.kubernetes.io/is-default-class")
                        .or_else(|| {
                            annotations.get("storageclass.beta.kubernetes.io/is-default-class")
                        })
                })
                .map(|v| v == "true")
                .unwrap_or(false);

            let status = if is_default {
                format!("{} (default)", provisioner)
            } else {
                provisioner
            };

            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: None,
                status,
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: Some(format!("{} / {}", reclaim_policy, binding_mode)),
                restarts: None,
            }
        })
        .collect()
}

pub fn events_to_items(events: &[Event]) -> Vec<ResourceItem> {
    events
        .iter()
        .map(|event| {
            let metadata = &event.metadata;
            let event_type = event.type_.clone().unwrap_or_else(|| "Normal".to_string());
            let reason = event.reason.clone().unwrap_or_default();
            let message = event.message.clone().unwrap_or_default();
            let involved = &event.involved_object;
            let object_name = format!(
                "{}/{}",
                involved
                    .kind
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                involved
                    .name
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string())
            );
            ResourceItem {
                name: object_name,
                namespace: metadata.namespace.clone(),
                status: event_type,
                age: reason,
                age_seconds: None,
                ready: Some(message),
                restarts: event.count.map(|c| c as u32),
            }
        })
        .collect()
}

pub fn nodes_to_items(nodes: &[Node]) -> Vec<ResourceItem> {
    nodes
        .iter()
        .map(|node| {
            let metadata = &node.metadata;
            let is_ready = node
                .status
                .as_ref()
                .and_then(|s| s.conditions.as_ref())
                .map(|conditions| {
                    conditions
                        .iter()
                        .any(|c| c.type_ == "Ready" && c.status == "True")
                })
                .unwrap_or(false);
            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: None,
                status: if is_ready {
                    "Ready".to_string()
                } else {
                    "NotReady".to_string()
                },
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: None,
                restarts: None,
            }
        })
        .collect()
}

pub fn roles_to_items(roles: &[Role]) -> Vec<ResourceItem> {
    roles
        .iter()
        .map(|role| {
            let metadata = &role.metadata;
            let rule_count = role.rules.as_ref().map(|r| r.len()).unwrap_or(0);
            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: format!("{} rules", rule_count),
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: None,
                restarts: None,
            }
        })
        .collect()
}

pub fn clusterroles_to_items(clusterroles: &[ClusterRole]) -> Vec<ResourceItem> {
    clusterroles
        .iter()
        .map(|role| {
            let metadata = &role.metadata;
            let rule_count = role.rules.as_ref().map(|r| r.len()).unwrap_or(0);
            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: None,
                status: format!("{} rules", rule_count),
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: None,
                restarts: None,
            }
        })
        .collect()
}

pub fn rolebindings_to_items(rolebindings: &[RoleBinding]) -> Vec<ResourceItem> {
    rolebindings
        .iter()
        .map(|rb| {
            let metadata = &rb.metadata;
            let role_ref = &rb.role_ref;
            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: metadata.namespace.clone(),
                status: format!("{}/{}", role_ref.kind, role_ref.name),
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: None,
                restarts: None,
            }
        })
        .collect()
}

pub fn clusterrolebindings_to_items(crbs: &[ClusterRoleBinding]) -> Vec<ResourceItem> {
    crbs.iter()
        .map(|crb| {
            let metadata = &crb.metadata;
            let role_ref = &crb.role_ref;
            ResourceItem {
                name: metadata.name.clone().unwrap_or_default(),
                namespace: None,
                status: format!("{}/{}", role_ref.kind, role_ref.name),
                age: format_age(metadata.creation_timestamp.as_ref()),
                age_seconds: age_seconds(metadata.creation_timestamp.as_ref()),
                ready: None,
                restarts: None,
            }
        })
        .collect()
}
