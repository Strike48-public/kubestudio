//! Shared session state for user auth tokens.
//!
//! Populated by the connector (from the `__st` query param) and read
//! by Dioxus components that need to make authenticated GraphQL requests.

use std::sync::{LazyLock, RwLock};

static USER_AUTH_TOKEN: LazyLock<RwLock<String>> = LazyLock::new(|| RwLock::new(String::new()));
static USER_DISPLAY_NAME: LazyLock<RwLock<String>> = LazyLock::new(|| RwLock::new(String::new()));
static TENANT_ID: LazyLock<RwLock<String>> = LazyLock::new(|| RwLock::new(String::new()));

pub fn get_auth_token() -> String {
    USER_AUTH_TOKEN
        .read()
        .map(|t| t.clone())
        .unwrap_or_default()
}

pub fn set_auth_token(token: &str) {
    if let Ok(mut t) = USER_AUTH_TOKEN.write() {
        *t = token.to_string();
    }
}

pub fn get_display_name() -> String {
    USER_DISPLAY_NAME
        .read()
        .map(|n| n.clone())
        .unwrap_or_default()
}

pub fn set_display_name(name: &str) {
    if let Ok(mut n) = USER_DISPLAY_NAME.write() {
        *n = name.to_string();
    }
}

/// Read the current tenant/realm name (e.g. "non-prod").
pub fn get_tenant_id() -> String {
    TENANT_ID.read().map(|t| t.clone()).unwrap_or_default()
}

/// Store the tenant/realm name.
pub fn set_tenant_id(tenant: &str) {
    if let Ok(mut t) = TENANT_ID.write() {
        *t = tenant.to_string();
    }
}
