use crate::utils::error::Result;
use std::collections::HashMap;

/// Resolved secret material.
///
/// For now this is a simple in-memory string; in the future this could be
/// extended to support binary data or streaming.
#[derive(Debug, Clone)]
pub struct SecretMaterial {
    /// The secret value as a string.
    pub value: String,
}

/// Trait implemented by concrete secret backends.
pub trait SecretBackend: Send + Sync {
    /// Logical name of this backend (e.g., "env", "age").
    fn name(&self) -> &'static str;

    /// Resolve a logical key to secret material.
    ///
    /// # Errors
    ///
    /// Returns an error if the secret cannot be resolved by this backend.
    fn resolve(&self, key: &str) -> Result<SecretMaterial>;

    /// Whether this backend can handle the given backend identifier.
    fn supports_backend(&self, backend: &str) -> bool {
        backend.eq_ignore_ascii_case(self.name())
    }
}

/// A very small backend that sources secrets from environment variables.
///
/// This is primarily useful for CI/testing and provides a simple, portable
/// default implementation.
pub struct EnvSecretBackend;

impl SecretBackend for EnvSecretBackend {
    fn name(&self) -> &'static str {
        "env"
    }

    fn resolve(&self, key: &str) -> Result<SecretMaterial> {
        let value = match std::env::var(key) {
            Ok(value) => value,
            Err(error) => {
                return Err(crate::utils::error::DotfilesError::Config(format!(
                    "Failed to read secret from environment variable '{key}': {error}"
                )));
            }
        };
        Ok(SecretMaterial { value })
    }
}

/// Manager that dispatches to the appropriate backend.
pub struct SecretsManager {
    /// List of secret backends available for resolution.
    backends: Vec<Box<dyn SecretBackend>>,
}

impl SecretsManager {
    /// Create a manager with the default set of backends.
    #[must_use]
    pub fn with_defaults() -> Self {
        let backends: Vec<Box<dyn SecretBackend>> = vec![Box::new(EnvSecretBackend)];
        Self { backends }
    }

    /// Resolve a secret using the requested backend (or fall back to any that
    /// support the key).
    ///
    /// # Errors
    ///
    /// Returns an error if no backend can resolve the secret, or if the resolution fails.
    pub fn resolve(&self, backend: Option<&str>, key: &str) -> Result<SecretMaterial> {
        // Try specific backend first if provided.
        if let Some(backend_name) = backend {
            for be in &self.backends {
                if be.supports_backend(backend_name) {
                    return be.resolve(key);
                }
            }
        }

        // Fallback: first backend that can resolve without explicit match.
        for be in &self.backends {
            if be.supports_backend(be.name()) {
                return be.resolve(key);
            }
        }

        Err(crate::utils::error::DotfilesError::Config(format!(
            "No secret backend available that can resolve key '{key}'"
        )))
    }

    /// Convenience helper to resolve multiple secrets into a map.
    ///
    /// # Errors
    ///
    /// Returns an error if any secret cannot be resolved.
    pub fn resolve_many(
        &self,
        backend: Option<&str>,
        keys: &[String],
    ) -> Result<HashMap<String, SecretMaterial>> {
        let mut out = HashMap::new();
        for key in keys {
            let mat = self.resolve(backend, key)?;
            out.insert(key.clone(), mat);
        }
        Ok(out)
    }
}
