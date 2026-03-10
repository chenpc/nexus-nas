use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use libnexus::proto::{
    nexus_service_client::NexusServiceClient, CommandRequest, CommandResponse,
    ListServicesRequest, ListServicesResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

type Client = Arc<Mutex<NexusServiceClient<Channel>>>;

#[derive(Clone)]
struct AppState {
    client: Client,
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

    let state = AppState { client };

    let app = Router::new()
        .route("/", get(index))
        .route("/index.html", get(index))
        .route("/api/services", get(list_services))
        .route("/api/execute", post(execute))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Nexus Web UI listening on http://{}", addr);
    println!("Connected to gRPC at {}", grpc_endpoint);

    axum::serve(listener, app).await?;
    Ok(())
}
