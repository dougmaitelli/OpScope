//! Adapters for external monitoring sources.

use crate::application::{ConnectionValidationFailure, SourceRegistry};
use std::sync::Arc;

pub mod fake;
pub mod github;

/// Builds the compiled source catalog shared by desktop and server editions.
pub fn registered_sources() -> Result<SourceRegistry, ConnectionValidationFailure> {
    Ok(SourceRegistry::new(vec![Arc::new(
        github::GitHubClient::new()?,
    )]))
}
