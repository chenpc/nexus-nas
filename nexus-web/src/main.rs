use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use libnexus::proto::{
    nexus_service_client::NexusServiceClient, CommandRequest, CommandResponse,
    ListServicesRequest, ListServicesResponse,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

type Client = Arc<Mutex<NexusServiceClient<Channel>>>;
type SessionStore = Arc<Mutex<HashMap<String, String>>>;

#[derive(Clone)]
struct AppState {
    client: Client,
    sessions: SessionStore,
}

#[derive(Deserialize)]
struct ExecuteRequest {
    service: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Serialize)]
struct ExecuteResponse {
    success: bool,
    message: String,
}

#[derive(Serialize)]
struct ArgDef {
    name: String,
    hint: String,
    description: String,
}

#[derive(Serialize)]
struct CommandDef {
    name: String,
    description: String,
    args: Vec<ArgDef>,
}

#[derive(Serialize)]
struct ServiceInfo {
    name: String,
    description: String,
    commands: Vec<CommandDef>,
}

#[derive(Serialize)]
struct UserInfo {
    username: String,
    uid: u32,
    comment: String,
}

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
    password: String,
    #[serde(default)]
    comment: String,
}

#[derive(Deserialize)]
struct ChangePasswordRequest {
    password: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn list_services(State(state): State<AppState>) -> impl IntoResponse {
    let mut guard = state.client.lock().await;
    let result: Result<tonic::Response<ListServicesResponse>, tonic::Status> =
        guard.list_services(tonic::Request::new(ListServicesRequest {})).await;
    match result {
        Ok(resp) => {
            let body: ListServicesResponse = resp.into_inner();
            let services: Vec<ServiceInfo> = body
                .services
                .into_iter()
                .map(|s| ServiceInfo {
                    name: s.name,
                    description: s.description,
                    commands: s
                        .commands
                        .into_iter()
                        .map(|c| CommandDef {
                            name: c.name,
                            description: c.description,
                            args: c
                                .args
                                .into_iter()
                                .map(|a| ArgDef {
                                    name: a.name,
                                    hint: a.hint,
                                    description: a.description,
                                })
                                .collect(),
                        })
                        .collect(),
                })
                .collect();
            Json(serde_json::json!({"services": services})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{}", e)})),
        )
            .into_response(),
    }
}

async fn execute(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> impl IntoResponse {
    let grpc_req = CommandRequest {
        service: req.service,
        action: req.command,
        args: req.args,
    };
    let mut guard = state.client.lock().await;
    let result: Result<tonic::Response<CommandResponse>, tonic::Status> =
        guard.execute(tonic::Request::new(grpc_req)).await;
    match result {
        Ok(resp) => {
            let r: CommandResponse = resp.into_inner();
            Json(ExecuteResponse {
                success: r.success,
                message: r.message,
            })
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ExecuteResponse {
                success: false,
                message: format!("{}", e),
            }),
        )
            .into_response(),
    }
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn login_page() -> Html<&'static str> {
    Html(include_str!("login.html"))
}

// Authentication middleware
async fn auth_middleware(
    jar: CookieJar,
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Redirect> {
    if let Some(cookie) = jar.get("nexus_session") {
        let sessions = state.sessions.lock().await;
        if sessions.contains_key(cookie.value()) {
            return Ok(next.run(request).await);
        }
    }
    Err(Redirect::to("/login"))
}

// Password verification
fn verify_password(username: &str, password: &str) -> bool {
    // Read /etc/shadow to find the user's password hash
    let shadow_content = match std::fs::read_to_string("/etc/shadow") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read /etc/shadow: {}", e);
            return false;
        }
    };

    for line in shadow_content.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 2 {
            continue;
        }
        if parts[0] != username {
            continue;
        }
        let hash = parts[1];
        // Empty password or disabled account
        if hash.is_empty() || hash == "!" || hash == "*" || hash == "!!" {
            return false;
        }
        // Verify using pwhash crate (supports $6$ SHA-512, $5$ SHA-256, $1$ MD5)
        return pwhash::unix::verify(password, hash);
    }
    false
}

// Generate random session token
fn generate_token() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    hex::encode(bytes)
}

// Login handler
async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> Result<(CookieJar, Redirect), StatusCode> {
    if verify_password(&req.username, &req.password) {
        let token = generate_token();
        let mut sessions = state.sessions.lock().await;
        sessions.insert(token.clone(), req.username);

        let cookie = Cookie::build(("nexus_session", token))
            .path("/")
            .http_only(true)
            .build();

        Ok((jar.add(cookie), Redirect::to("/")))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

// Logout handler
async fn logout(State(state): State<AppState>, jar: CookieJar) -> (CookieJar, Redirect) {
    if let Some(cookie) = jar.get("nexus_session") {
        let mut sessions = state.sessions.lock().await;
        sessions.remove(cookie.value());
    }

    let cookie = Cookie::build(("nexus_session", ""))
        .path("/")
        .build();

    (jar.remove(cookie), Redirect::to("/login"))
}

// List users
async fn list_users() -> impl IntoResponse {
    let content = match fs::read_to_string("/etc/passwd") {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read passwd: {}", e)})),
            )
                .into_response()
        }
    };

    let users: Vec<UserInfo> = content
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 5 {
                let username = parts[0].to_string();
                let uid: u32 = parts[2].parse().ok()?;
                let comment = parts[4].to_string();

                // Include root (uid 0) or normal users (uid >= 1000)
                if uid == 0 || uid >= 1000 {
                    return Some(UserInfo {
                        username,
                        uid,
                        comment,
                    });
                }
            }
            None
        })
        .collect();

    Json(serde_json::json!({"users": users})).into_response()
}

// Create user
async fn create_user(Json(req): Json<CreateUserRequest>) -> impl IntoResponse {
    // Create user with useradd
    let mut cmd = Command::new("useradd");
    cmd.arg("-m");
    if !req.comment.is_empty() {
        cmd.arg("-c").arg(&req.comment);
    }
    cmd.arg(&req.username);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to execute useradd: {}", e)})),
            )
                .into_response()
        }
    };

    if !output.status.success() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("useradd failed: {}", String::from_utf8_lossy(&output.stderr))
            })),
        )
            .into_response();
    }

    // Set password using chpasswd (reads from stdin)
    let chpasswd_input = format!("{}:{}", req.username, req.password);
    let output = match Command::new("sh")
        .arg("-c")
        .arg(format!("echo '{}' | chpasswd", chpasswd_input))
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to set password: {}", e)})),
            )
                .into_response()
        }
    };

    if !output.status.success() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("chpasswd failed: {}", String::from_utf8_lossy(&output.stderr))
            })),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({"success": true})),
    )
        .into_response()
}

// Delete user
async fn delete_user(Path(username): Path<String>) -> impl IntoResponse {
    let output = match Command::new("userdel").arg("-r").arg(&username).output() {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to execute userdel: {}", e)})),
            )
                .into_response()
        }
    };

    if output.status.success() {
        Json(serde_json::json!({"success": true})).into_response()
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("userdel failed: {}", String::from_utf8_lossy(&output.stderr))
            })),
        )
            .into_response()
    }
}

// Change password
async fn change_password(
    Path(username): Path<String>,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    let chpasswd_input = format!("{}:{}", username, req.password);
    let output = match Command::new("sh")
        .arg("-c")
        .arg(format!("echo '{}' | chpasswd", chpasswd_input))
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to set password: {}", e)})),
            )
                .into_response()
        }
    };

    if output.status.success() {
        Json(serde_json::json!({"success": true})).into_response()
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("chpasswd failed: {}", String::from_utf8_lossy(&output.stderr))
            })),
        )
            .into_response()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());

    let grpc_endpoint = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "http://[::1]:9000".to_string());

    let channel = Channel::from_shared(grpc_endpoint.clone())?
        .connect()
        .await?;
    let client: Arc<Mutex<NexusServiceClient<Channel>>> =
        Arc::new(Mutex::new(NexusServiceClient::new(channel)));

    let sessions: SessionStore = Arc::new(Mutex::new(HashMap::new()));

    let state = AppState { client, sessions };

    // Protected routes (require authentication)
    let protected = Router::new()
        .route("/", get(index))
        .route("/index.html", get(index))
        .route("/api/services", get(list_services))
        .route("/api/execute", post(execute))
        .route("/api/users", get(list_users))
        .route("/api/users", post(create_user))
        .route("/api/users/:username", delete(delete_user))
        .route("/api/users/:username/passwd", post(change_password))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // Public routes (no authentication)
    let public = Router::new()
        .route("/login", get(login_page))
        .route("/login", post(login))
        .route("/logout", get(logout));

    let app = Router::new()
        .merge(protected)
        .merge(public)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Nexus Web UI listening on http://{}", addr);
    println!("Connected to gRPC at {}", grpc_endpoint);

    axum::serve(listener, app).await?;
    Ok(())
}
