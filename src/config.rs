use std::path::PathBuf;
use std::sync::OnceLock;

static QUIET: OnceLock<bool> = OnceLock::new();

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub fn is_quiet() -> bool {
    *QUIET.get_or_init(|| env_flag("SYNAPSIS_QUIET") || env_flag("QUIET"))
}

pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SYNAPSIS_DATA_DIR") {
        PathBuf::from(dir)
    } else {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapsis")
    }
}

pub fn port() -> u16 {
    std::env::var("SYNAPSIS_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|port| *port != 0)
        .unwrap_or(7438)
}

pub fn bind_host() -> String {
    std::env::var("SYNAPSIS_BIND_HOST")
        .ok()
        .map(|host| host.trim().to_string())
        .filter(|host| !host.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

/// Formats a host and port for TCP binding, including IPv6 bracket notation.
pub fn bind_addr(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

pub fn log_level() -> String {
    std::env::var("SYNAPSIS_LOG").unwrap_or_else(|_| "info".to_string())
}

pub fn db_key() -> Option<Vec<u8>> {
    if let Ok(hex_key) = std::env::var("SYNAPSIS_DB_KEY") {
        hex::decode(&hex_key).ok().filter(|k| !k.is_empty())
    } else if let Ok(b64_key) = std::env::var("SYNAPSIS_DB_KEY_BASE64") {
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64_key)
            .ok()
            .filter(|k| !k.is_empty())
    } else {
        None
    }
}

pub fn insecure_tls() -> bool {
    env_flag("SYNAPSIS_INSECURE_TLS")
}

pub fn allow_private_mcp() -> bool {
    env_flag("SYNAPSIS_ALLOW_PRIVATE_MCP")
}

pub fn allow_dangerous_shell() -> bool {
    env_flag("SYNAPSIS_ALLOW_DANGEROUS_SHELL")
}

pub fn auth_enabled() -> bool {
    env_flag("SYNAPSIS_AUTH")
}

pub fn secret_key() -> Option<String> {
    std::env::var("SYNAPSIS_SECRET_KEY")
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

pub fn api_keys() -> Vec<String> {
    std::env::var("SYNAPSIS_API_KEYS")
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::bind_addr;

    #[test]
    fn formats_ipv4_bind_address() {
        assert_eq!(bind_addr("127.0.0.1", 7438), "127.0.0.1:7438");
    }

    #[test]
    fn formats_ipv6_bind_address() {
        assert_eq!(bind_addr("::1", 7438), "[::1]:7438");
        assert_eq!(bind_addr("[::1]", 7438), "[::1]:7438");
    }
}
