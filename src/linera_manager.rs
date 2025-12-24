use anyhow::{anyhow, Result};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::models::{OwnerAddResult, WalletInfo};

/// Default faucet URL for Linera testnet
pub const DEFAULT_FAUCET_URL: &str = "https://faucet.testnet-conway.linera.net";

/// Manages the Linera service process and wallet operations
pub struct LineraManager {
    /// Path to the linera binary
    linera_bin: String,
    /// Current service process (if running)
    service_process: Arc<RwLock<Option<Child>>>,
    /// Port the service is running on
    service_port: Arc<RwLock<Option<u16>>>,
    /// Path to wallet file
    wallet_path: String,
    /// Path to keystore file
    keystore_path: String,
}

impl LineraManager {
    pub fn new() -> Self {
        // Default paths - can be configured
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        
        Self {
            linera_bin: std::env::var("LINERA_BIN")
                .unwrap_or_else(|_| format!("{}/bin/linera", home)),
            service_process: Arc::new(RwLock::new(None)),
            service_port: Arc::new(RwLock::new(None)),
            wallet_path: std::env::var("LINERA_WALLET")
                .unwrap_or_else(|_| format!("{}/linera-wallet.json", home)),
            keystore_path: std::env::var("LINERA_KEYSTORE")
                .unwrap_or_else(|_| format!("{}/linera-keystore.json", home)),
        }
    }

    /// Check if the service is currently running
    pub async fn is_running(&self) -> bool {
        let process = self.service_process.read().await;
        process.is_some()
    }

    /// Get the current service PID if running
    pub async fn get_pid(&self) -> Option<u32> {
        let process = self.service_process.read().await;
        process.as_ref().and_then(|p| p.id())
    }

    /// Get the current service port if running
    pub async fn get_port(&self) -> Option<u16> {
        *self.service_port.read().await
    }

    /// Initialize a new wallet using the faucet
    pub async fn init_wallet(&self, faucet_url: Option<&str>) -> Result<WalletInfo> {
        let faucet = faucet_url.unwrap_or(DEFAULT_FAUCET_URL);
        
        info!("Initializing wallet with faucet: {}", faucet);

        // Run: linera wallet init --faucet <url>
        let output = Command::new(&self.linera_bin)
            .args([
                "--wallet", &self.wallet_path,
                "--keystore", &self.keystore_path,
                "wallet", "init",
                "--faucet", faucet,
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to init wallet: {}", stderr);
            return Err(anyhow!("Failed to initialize wallet: {}", stderr));
        }

        // Get wallet info
        self.get_wallet_info().await
    }

    /// Get current wallet information
    pub async fn get_wallet_info(&self) -> Result<WalletInfo> {
        // Run: linera wallet show
        let output = Command::new(&self.linera_bin)
            .args([
                "--wallet", &self.wallet_path,
                "--keystore", &self.keystore_path,
                "wallet", "show",
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to get wallet info: {}", stderr));
        }

        // Combine stdout and stderr since linera writes info to both
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stderr, stdout);
        
        // Parse the output to extract chain_id
        let chain_id = self.extract_field_flexible(&combined, "Chain ID")?;
        
        // Public key may not exist for faucet-created wallets
        let public_key = self.extract_field_flexible(&combined, "Default owner")
            .unwrap_or_else(|_| "No owner key".to_string());

        Ok(WalletInfo {
            chain_id,
            public_key,
        })
    }

    /// Start the linera service
    pub async fn start_service(&self, port: u16) -> Result<()> {
        // Check if already running
        if self.is_running().await {
            return Err(anyhow!("Service is already running"));
        }

        info!("Starting linera service on port {}", port);

        // Storage using memory (no persistence)
        let storage = "memory";

        // Spawn: linera service --port <port>
        let child = Command::new(&self.linera_bin)
            .args([
                "--wallet", &self.wallet_path,
                "--keystore", &self.keystore_path,
                "--storage", storage,
                "service",
                "--port", &port.to_string(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        *self.service_process.write().await = Some(child);
        *self.service_port.write().await = Some(port);

        // Give it a moment to start
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        info!("Linera service started on port {}", port);
        Ok(())
    }

    /// Stop the linera service
    pub async fn stop_service(&self) -> Result<()> {
        let mut process = self.service_process.write().await;
        
        if let Some(mut child) = process.take() {
            info!("Stopping linera service");
            child.kill().await?;
            child.wait().await?;
            *self.service_port.write().await = None;
            info!("Linera service stopped");
            Ok(())
        } else {
            Err(anyhow!("Service is not running"))
        }
    }

    /// Add owners to a chain
    pub async fn add_owner(&self, chain_id: &str, public_keys: Vec<String>) -> Result<OwnerAddResult> {
        if public_keys.is_empty() {
            return Err(anyhow!("At least one public key is required"));
        }

        info!("Adding {} owners to chain {}", public_keys.len(), chain_id);

        // Build args: linera change-ownership --chain-id <id> --owner-public-keys <key1> <key2> ...
        let mut args = vec![
            "--wallet".to_string(), self.wallet_path.clone(),
            "--keystore".to_string(), self.keystore_path.clone(),
            "change-ownership".to_string(),
            "--chain-id".to_string(), chain_id.to_string(),
        ];
        
        for key in &public_keys {
            args.push("--owner-public-keys".to_string());
            args.push(key.clone());
        }

        let output = Command::new(&self.linera_bin)
            .args(&args)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to add owner: {}", stderr);
            return Err(anyhow!("Failed to add owner: {}", stderr));
        }

        Ok(OwnerAddResult {
            success: true,
            chain_id: chain_id.to_string(),
            owners: public_keys,
        })
    }

    /// Generate a new keypair and return the public key
    pub async fn keygen(&self) -> Result<String> {
        let output = Command::new(&self.linera_bin)
            .args([
                "--wallet", &self.wallet_path,
                "--keystore", &self.keystore_path,
                "keygen",
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to generate keypair: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // The public key is usually printed directly
        Ok(stdout.trim().to_string())
    }

    /// Helper to extract field from linera output
    fn extract_field(&self, output: &str, field: &str) -> Result<String> {
        for line in output.lines() {
            if line.contains(field) {
                // Try to extract the value after the colon or equals
                if let Some(value) = line.split(':').nth(1) {
                    return Ok(value.trim().to_string());
                }
                if let Some(value) = line.split('=').nth(1) {
                    return Ok(value.trim().to_string());
                }
            }
        }
        
        // If not found with label, try to find a hex string that looks like a chain ID or key
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.len() >= 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(trimmed.to_string());
            }
        }
        
        Err(anyhow!("Could not find {} in output", field))
    }

    /// Helper to extract field from linera output (flexible with multiple spaces)
    fn extract_field_flexible(&self, output: &str, field: &str) -> Result<String> {
        for line in output.lines() {
            if line.contains(field) {
                // Split by field name and take what comes after
                if let Some(rest) = line.split(field).nth(1) {
                    // Remove leading colons, spaces, etc.
                    let value = rest.trim_start_matches(':').trim();
                    if !value.is_empty() {
                        return Ok(value.to_string());
                    }
                }
            }
        }
        
        // Fallback to original method
        self.extract_field(output, field)
    }
}

impl Default for LineraManager {
    fn default() -> Self {
        Self::new()
    }
}
