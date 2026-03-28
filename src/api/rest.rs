//! HTTP REST API for Synapsis

use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use warp::{Filter, Rejection, Reply};

/// API response wrapper
#[derive(Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Health check endpoint
pub async fn health_check() -> Result<impl Reply, Rejection> {
    Ok(warp::reply::json(&ApiResponse::success("OK")))
}

/// System status endpoint
pub async fn system_status() -> Result<impl Reply, Rejection> {
    let status = serde_json::json!({
        "status": "operational",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    Ok(warp::reply::json(&ApiResponse::success(status)))
}

/// Agent registration request
#[derive(Deserialize)]
pub struct RegisterAgentRequest {
    pub agent_id: String,
    pub agent_type: String,
    pub capabilities: Vec<String>,
}

/// Agent registration endpoint
pub async fn register_agent(_req: RegisterAgentRequest) -> Result<impl Reply, Rejection> {
    // TODO: Integrate with actual agent registry
    let response = serde_json::json!({
        "agent_id": _req.agent_id,
        "status": "registered",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    Ok(warp::reply::json(&ApiResponse::success(response)))
}

/// Task submission request
#[derive(Deserialize)]
pub struct SubmitTaskRequest {
    pub task_type: String,
    pub payload: serde_json::Value,
    pub priority: Option<u8>,
}

/// Task submission endpoint
pub async fn submit_task(_req: SubmitTaskRequest) -> Result<impl Reply, Rejection> {
    // TODO: Integrate with task queue
    let response = serde_json::json!({
        "task_id": uuid::Uuid::new_v4().to_string(),
        "status": "queued",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    Ok(warp::reply::json(&ApiResponse::success(response)))
}

/// Get all routes
pub fn routes() -> impl Filter<Extract = impl Reply, Error = Infallible> + Clone {
    let health = warp::path!("health")
        .and(warp::get())
        .and_then(health_check);

    let status = warp::path!("status")
        .and(warp::get())
        .and_then(system_status);

    let register = warp::path!("agent" / "register")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(register_agent);

    let submit = warp::path!("task" / "submit")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(submit_task);

    health
        .or(status)
        .or(register)
        .or(submit)
        .with(warp::cors().allow_any_origin())
        .with(warp::log("api"))
        .recover(handle_rejection)
}

/// Handle rejections and return proper JSON error responses
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (warp::http::StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        (
            warp::http::StatusCode::BAD_REQUEST,
            format!("Bad Request: {}", e),
        )
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        (
            warp::http::StatusCode::METHOD_NOT_ALLOWED,
            "Method Not Allowed".to_string(),
        )
    } else {
        eprintln!("Unhandled rejection: {:?}", err);
        (
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
    };

    let json = warp::reply::json(&ApiResponse::<()>::error(message));
    Ok(warp::reply::with_status(json, code))
}
