use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use tracing::{error, info};

/// GraphQL proxy for forwarding requests to linera service
pub struct GraphQLProxy {
    client: Client,
    base_url: String,
}

impl GraphQLProxy {
    pub fn new(port: u16) -> Self {
        Self {
            client: Client::new(),
            base_url: format!("http://localhost:{}", port),
        }
    }

    /// Proxy a GraphQL query to the linera service
    pub async fn query(
        &self,
        chain_id: &str,
        app_id: Option<&str>,
        query: &str,
        variables: Option<Value>,
    ) -> Result<Value> {
        // Build the URL
        let url = if let Some(app) = app_id {
            format!("{}/chains/{}/applications/{}", self.base_url, chain_id, app)
        } else {
            format!("{}/chains/{}", self.base_url, chain_id)
        };

        info!("Proxying GraphQL query to: {}", url);

        // Build the GraphQL request body
        let mut body = serde_json::json!({
            "query": query
        });

        if let Some(vars) = variables {
            body["variables"] = vars;
        }

        // Send the request
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let response_body: Value = response.json().await?;

        if !status.is_success() {
            error!("GraphQL request failed with status {}: {:?}", status, response_body);
        }

        Ok(response_body)
    }

    /// Query the system API (no chain/app)
    pub async fn system_query(&self, query: &str, variables: Option<Value>) -> Result<Value> {
        let url = format!("{}/", self.base_url);

        info!("Proxying system GraphQL query to: {}", url);

        let mut body = serde_json::json!({
            "query": query
        });

        if let Some(vars) = variables {
            body["variables"] = vars;
        }

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        Ok(response.json().await?)
    }

    /// Check if the linera service is reachable
    pub async fn health_check(&self) -> bool {
        let url = format!("{}/", self.base_url);
        
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }
}
