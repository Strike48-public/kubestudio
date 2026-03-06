// ks-core: Core types and error handling for KubeStudio
// This crate has no dependencies on UI or Kubernetes client internals

pub mod error;

pub use error::{SkdError, SkdResult};
