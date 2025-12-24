mod graphql_proxy;
mod linera_manager;
mod models;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::graphql_proxy::GraphQLProxy;
use crate::linera_manager::LineraManager;
use crate::models::*;

/// Application state shared across handlers
pub struct AppState {
    pub manager: LineraManager,
    pub graphql_proxy: RwLock<Option<GraphQLProxy>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            manager: LineraManager::new(),
            graphql_proxy: RwLock::new(None),
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    info!("Starting Linera REST API Server");

    // Create shared state
    let state = Arc::new(AppState::new());

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Static file serving for web UI
    let web_dir = std::env::var("WEB_DIR").unwrap_or_else(|_| "./web".to_string());
    
    // Build router
    let app = Router::new()
        // Service management
        .route("/service/start", post(start_service))
        .route("/service/stop", post(stop_service))
        .route("/service/status", get(get_status))
        // Wallet management
        .route("/wallet/init", post(init_wallet))
        .route("/wallet/info", get(get_wallet_info))
        .route("/wallet/keygen", post(keygen))
        // Owner management
        .route("/owner/add", post(add_owner))
        // GraphQL proxy
        .route("/graphql", post(proxy_graphql))
        .route("/graphql/system", post(proxy_system_graphql))
        // Health check
        .route("/health", get(health_check))
        // Serve static files (web UI)
        .fallback_service(ServeDir::new(&web_dir))
        .layer(cors)
        .with_state(state);

    // Start server
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let addr = format!("0.0.0.0:{}", port);
    info!("Listening on {}", addr);
    info!("Web UI available at http://localhost:{}/", port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Start the linera service
async fn start_service(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartServiceRequest>,
) -> impl IntoResponse {
    let port = req.port;

    match state.manager.start_service(port).await {
        Ok(()) => {
            // Initialize GraphQL proxy with the service port
            *state.graphql_proxy.write().await = Some(GraphQLProxy::new(port));
            
            (
                StatusCode::OK,
                Json(ApiResponse::success(serde_json::json!({
                    "status": "started",
                    "port": port
                }))),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::error(e.to_string())),
        ),
    }
}

/// Stop the linera service
async fn stop_service(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.manager.stop_service().await {
        Ok(()) => {
            *state.graphql_proxy.write().await = None;
            
            (
                StatusCode::OK,
                Json(ApiResponse::success(serde_json::json!({
                    "status": "stopped"
                }))),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::error(e.to_string())),
        ),
    }
}

/// Get service status
async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let running = state.manager.is_running().await;
    let pid = state.manager.get_pid().await;
    let port = state.manager.get_port().await;

    Json(ApiResponse::success(ServiceStatus { running, pid, port }))
}

/// Initialize wallet with faucet
async fn init_wallet(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InitWalletRequest>,
) -> impl IntoResponse {
    match state.manager.init_wallet(Some(&req.faucet_url)).await {
        Ok(info) => (StatusCode::OK, Json(ApiResponse::success(info))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<WalletInfo>::error(e.to_string())),
        ),
    }
}

/// Get current wallet info
async fn get_wallet_info(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.manager.get_wallet_info().await {
        Ok(info) => (StatusCode::OK, Json(ApiResponse::success(info))),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<WalletInfo>::error(e.to_string())),
        ),
    }
}

/// Generate new keypair
async fn keygen(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.manager.keygen().await {
        Ok(public_key) => (
            StatusCode::OK,
            Json(ApiResponse::success(serde_json::json!({
                "public_key": public_key
            }))),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::error(e.to_string())),
        ),
    }
}

/// Add owner to chain
async fn add_owner(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddOwnerRequest>,
) -> impl IntoResponse {
    match state.manager.add_owner(&req.chain_id, req.public_keys).await {
        Ok(result) => (StatusCode::OK, Json(ApiResponse::success(result))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<OwnerAddResult>::error(e.to_string())),
        ),
    }
}

/// Proxy GraphQL query to linera service
async fn proxy_graphql(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GraphQLProxyRequest>,
) -> impl IntoResponse {
    let proxy = state.graphql_proxy.read().await;
    
    match proxy.as_ref() {
        Some(p) => {
            match p.query(&req.chain_id, req.app_id.as_deref(), &req.query, req.variables).await {
                Ok(result) => (StatusCode::OK, Json(result)),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": e.to_string()
                    })),
                ),
            }
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Linera service is not running"
            })),
        ),
    }
}

/// Proxy system GraphQL query
async fn proxy_system_graphql(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let proxy = state.graphql_proxy.read().await;
    
    match proxy.as_ref() {
        Some(p) => {
            let query = body["query"].as_str().unwrap_or("");
            let variables = body.get("variables").cloned();
            
            match p.system_query(query, variables).await {
                Ok(result) => (StatusCode::OK, Json(result)),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": e.to_string()
                    })),
                ),
            }
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Linera service is not running"
            })),
        ),
    }
}
