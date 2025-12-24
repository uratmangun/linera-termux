use serde::{Deserialize, Serialize};

/// Request to start linera service
#[derive(Debug, Deserialize)]
pub struct StartServiceRequest {
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    8080
}

/// Request to initialize wallet
#[derive(Debug, Deserialize)]
pub struct InitWalletRequest {
    pub faucet_url: String,
}

/// Request to add owner to chain
#[derive(Debug, Deserialize)]
pub struct AddOwnerRequest {
    pub chain_id: String,
    pub public_keys: Vec<String>,
}

/// Request to proxy GraphQL query
#[derive(Debug, Deserialize)]
pub struct GraphQLProxyRequest {
    pub chain_id: String,
    #[serde(default)]
    pub app_id: Option<String>,
    pub query: String,
    #[serde(default)]
    pub variables: Option<serde_json::Value>,
}

/// Generic API response
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Service status response
#[derive(Debug, Serialize)]
pub struct ServiceStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: Option<u16>,
}

/// Wallet initialization response
#[derive(Debug, Serialize)]
pub struct WalletInfo {
    pub chain_id: String,
    pub public_key: String,
}

/// Owner addition response
#[derive(Debug, Serialize)]
pub struct OwnerAddResult {
    pub success: bool,
    pub chain_id: String,
    pub owners: Vec<String>,
}
