use axum::{
    Router,
    extract::{Json, State},
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

/// Peer information stored by the tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub addr: String,
    pub last_seen: u64, // Unix timestamp
}

/// Request to announce a peer
#[derive(Debug, Deserialize, Serialize)]
pub struct AnnounceRequest {
    pub addr: String,
}

/// Response containing list of peers
#[derive(Debug, Serialize, Deserialize)]
pub struct PeersResponse {
    pub peers: Vec<PeerInfo>,
}

/// Standard API response format
#[derive(Debug, Serialize)]
pub struct ApiResponse {
    pub status: String,
}

/// Tracker state - stores information about connected peers
#[derive(Debug)]
pub struct TrackerState {
    peers: RwLock<HashMap<String, PeerInfo>>,
}

impl TrackerState {
    pub fn new() -> Self {
        Self {
            peers: RwLock::new(HashMap::new()),
        }
    }

    /// Add or update a peer in the tracker
    pub async fn announce_peer(&self, peer: PeerInfo) {
        let mut peers = self.peers.write().await;
        peers.insert(peer.addr.clone(), peer);
    }

    /// Get all active peers
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }

    /// Remove inactive peers (older than timeout_seconds)
    pub async fn cleanup_peers(&self, timeout_seconds: u64) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut peers = self.peers.write().await;
        peers.retain(|_, peer| current_time - peer.last_seen < timeout_seconds);
    }
}

/// HTTP tracker server
pub struct TrackerServer {
    state: Arc<TrackerState>,
}

impl TrackerServer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(TrackerState::new()),
        }
    }

    /// Build the axum router with all routes
    fn build_router(&self) -> Router {
        Router::new()
            .route("/", get(handle_root))
            .route("/announce", post(handle_announce))
            .route("/peers", get(handle_get_peers))
            .with_state(Arc::clone(&self.state))
    }

    /// Start the tracker server on the specified port
    pub async fn start(&self, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(addr).await?;

        println!("Tracker HTTP server listening on http://{}", addr);

        // Start cleanup task
        let state_for_cleanup = Arc::clone(&self.state);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                state_for_cleanup.cleanup_peers(300).await; // 5 minutes timeout
            }
        });

        let app = self.build_router();
        axum::serve(listener, app).await?;

        Ok(())
    }
}

/// Handle peer announcement
async fn handle_announce(
    State(state): State<Arc<TrackerState>>,
    Json(announce_req): Json<AnnounceRequest>,
) -> impl IntoResponse {
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let peer = PeerInfo {
        addr: announce_req.addr,
        last_seen: current_time,
    };

    state.announce_peer(peer).await;

    ResponseJson(ApiResponse {
        status: "ok".to_string(),
    })
}

/// Handle request to get all peers
async fn handle_get_peers(State(state): State<Arc<TrackerState>>) -> impl IntoResponse {
    let peers = state.get_peers().await;
    let response = PeersResponse { peers };

    ResponseJson(response)
}

/// Handle root path - show tracker info
async fn handle_root() -> impl IntoResponse {
    let info = serde_json::json!({
        "name": "P2PSync Tracker",
        "version": "1.0.0",
        "endpoints": {
            "announce": "POST /announce",
            "peers": "GET /peers"
        }
    });

    ResponseJson(info)
}
