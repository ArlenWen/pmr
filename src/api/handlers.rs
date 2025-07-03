#[cfg(feature = "http-api")]
use crate::{
    api::auth::AuthManager,
    database::ProcessRecord,
    process::ProcessManager,
    Error,
};
#[cfg(feature = "http-api")]
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
#[cfg(feature = "http-api")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "http-api")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "http-api")]
use std::collections::HashMap;
#[cfg(feature = "http-api")]
use utoipa::ToSchema;

// Helper function to validate authentication
#[cfg(feature = "http-api")]
fn validate_auth(headers: &HeaderMap, auth_manager: &Arc<Mutex<AuthManager>>) -> Result<(), StatusCode> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..];
    let auth_manager = auth_manager.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !auth_manager.validate_token_sync(token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(())
}

#[cfg(feature = "http-api")]
#[derive(Serialize, ToSchema)]
pub struct ApiResponse<T> {
    /// Whether the request was successful
    pub success: bool,
    /// Response data (present on success)
    pub data: Option<T>,
    /// Error message (present on failure)
    pub error: Option<String>,
}

// Specific response types for OpenAPI schema generation
#[cfg(feature = "http-api")]
#[derive(Serialize, ToSchema)]
pub struct ProcessListResponse {
    /// Whether the request was successful
    pub success: bool,
    /// List of processes (present on success)
    pub data: Option<Vec<ProcessRecord>>,
    /// Error message (present on failure)
    pub error: Option<String>,
}

#[cfg(feature = "http-api")]
#[derive(Serialize, ToSchema)]
pub struct ProcessResponse {
    /// Whether the request was successful
    pub success: bool,
    /// Process data (present on success)
    pub data: Option<ProcessRecord>,
    /// Error message (present on failure)
    pub error: Option<String>,
}

#[cfg(feature = "http-api")]
#[derive(Serialize, ToSchema)]
pub struct MessageResponse {
    /// Whether the request was successful
    pub success: bool,
    /// Response message (present on success)
    pub data: Option<String>,
    /// Error message (present on failure)
    pub error: Option<String>,
}

#[cfg(feature = "http-api")]
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

// Implementations for specific response types
#[cfg(feature = "http-api")]
impl ProcessListResponse {
    pub fn success(data: Vec<ProcessRecord>) -> Self {
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

#[cfg(feature = "http-api")]
impl ProcessResponse {
    pub fn success(data: ProcessRecord) -> Self {
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

#[cfg(feature = "http-api")]
impl MessageResponse {
    pub fn success(data: String) -> Self {
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

#[cfg(feature = "http-api")]
#[derive(Deserialize, ToSchema)]
pub struct StartProcessRequest {
    /// Process name (must be unique)
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables
    pub env_vars: Option<HashMap<String, String>>,
    /// Working directory (defaults to current directory)
    pub working_dir: Option<String>,
    /// Log directory (defaults to ./logs)
    pub log_dir: Option<String>,
}

#[cfg(feature = "http-api")]
#[derive(Deserialize, ToSchema)]
pub struct LogsQuery {
    /// Number of lines to return (default: all)
    pub lines: Option<usize>,
    /// Whether to return rotated log files
    pub rotated: Option<bool>,
}

#[cfg(feature = "http-api")]
#[utoipa::path(
    get,
    path = "/api/processes",
    responses(
        (status = 200, description = "List of all processes", body = ProcessListResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_processes(
    State((process_manager, auth_manager)): State<(Arc<ProcessManager>, Arc<Mutex<AuthManager>>)>,
    headers: HeaderMap,
) -> std::result::Result<Json<ProcessListResponse>, StatusCode> {
    validate_auth(&headers, &auth_manager)?;
    match process_manager.list_processes().await {
        Ok(processes) => Ok(Json(ProcessListResponse::success(processes))),
        Err(e) => {
            eprintln!("Error listing processes: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(feature = "http-api")]
#[utoipa::path(
    get,
    path = "/api/processes/{name}",
    responses(
        (status = 200, description = "Process status", body = ProcessResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Process not found")
    ),
    params(
        ("name" = String, Path, description = "Process name")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_process_status(
    State((process_manager, auth_manager)): State<(Arc<ProcessManager>, Arc<Mutex<AuthManager>>)>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> std::result::Result<Json<ProcessResponse>, StatusCode> {
    validate_auth(&headers, &auth_manager)?;
    match process_manager.get_process_status(&name).await {
        Ok(process) => Ok(Json(ProcessResponse::success(process))),
        Err(Error::ProcessNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("Error getting process status: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(feature = "http-api")]
#[utoipa::path(
    post,
    path = "/api/processes",
    request_body = StartProcessRequest,
    responses(
        (status = 200, description = "Process started successfully", body = MessageResponse),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Process already exists")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn start_process(
    State((process_manager, auth_manager)): State<(Arc<ProcessManager>, Arc<Mutex<AuthManager>>)>,
    headers: HeaderMap,
    Json(request): Json<StartProcessRequest>,
) -> std::result::Result<Json<MessageResponse>, StatusCode> {
    validate_auth(&headers, &auth_manager)?;
    let env_vars = request.env_vars.unwrap_or_default();

    match process_manager
        .start_process(
            &request.name,
            &request.command,
            request.args,
            env_vars,
            request.working_dir,
            request.log_dir,
        )
        .await
    {
        Ok(message) => Ok(Json(MessageResponse::success(message))),
        Err(Error::ProcessAlreadyExists(_)) => Err(StatusCode::CONFLICT),
        Err(e) => {
            eprintln!("Error starting process: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(feature = "http-api")]
#[utoipa::path(
    put,
    path = "/api/processes/{name}/stop",
    responses(
        (status = 200, description = "Process stopped successfully", body = MessageResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Process not found")
    ),
    params(
        ("name" = String, Path, description = "Process name")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn stop_process(
    State((process_manager, auth_manager)): State<(Arc<ProcessManager>, Arc<Mutex<AuthManager>>)>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> std::result::Result<Json<MessageResponse>, StatusCode> {
    validate_auth(&headers, &auth_manager)?;
    match process_manager.stop_process(&name).await {
        Ok(message) => Ok(Json(MessageResponse::success(message))),
        Err(Error::ProcessNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("Error stopping process: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(feature = "http-api")]
#[utoipa::path(
    put,
    path = "/api/processes/{name}/restart",
    responses(
        (status = 200, description = "Process restarted successfully", body = MessageResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Process not found")
    ),
    params(
        ("name" = String, Path, description = "Process name")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn restart_process(
    State((process_manager, auth_manager)): State<(Arc<ProcessManager>, Arc<Mutex<AuthManager>>)>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> std::result::Result<Json<MessageResponse>, StatusCode> {
    validate_auth(&headers, &auth_manager)?;
    match process_manager.restart_process(&name).await {
        Ok(message) => Ok(Json(MessageResponse::success(message))),
        Err(Error::ProcessNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("Error restarting process: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(feature = "http-api")]
#[utoipa::path(
    delete,
    path = "/api/processes/{name}",
    responses(
        (status = 200, description = "Process deleted successfully", body = MessageResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Process not found")
    ),
    params(
        ("name" = String, Path, description = "Process name")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_process(
    State((process_manager, auth_manager)): State<(Arc<ProcessManager>, Arc<Mutex<AuthManager>>)>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> std::result::Result<Json<MessageResponse>, StatusCode> {
    validate_auth(&headers, &auth_manager)?;
    match process_manager.delete_process(&name).await {
        Ok(message) => Ok(Json(MessageResponse::success(message))),
        Err(Error::ProcessNotFound(_)) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("Error deleting process: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(feature = "http-api")]
#[utoipa::path(
    get,
    path = "/api/processes/{name}/logs",
    responses(
        (status = 200, description = "Process logs", body = MessageResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Process not found")
    ),
    params(
        ("name" = String, Path, description = "Process name"),
        ("lines" = Option<usize>, Query, description = "Number of lines to return"),
        ("rotated" = Option<bool>, Query, description = "Whether to return rotated log files")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_process_logs(
    State((process_manager, auth_manager)): State<(Arc<ProcessManager>, Arc<Mutex<AuthManager>>)>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Query(params): Query<LogsQuery>,
) -> std::result::Result<Json<MessageResponse>, StatusCode> {
    validate_auth(&headers, &auth_manager)?;
    if params.rotated.unwrap_or(false) {
        match process_manager.get_rotated_logs(&name).await {
            Ok(logs) => Ok(Json(MessageResponse::success(logs.join("\n")))),
            Err(Error::ProcessNotFound(_)) => Err(StatusCode::NOT_FOUND),
            Err(e) => {
                eprintln!("Error getting rotated logs: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        match process_manager.get_process_logs(&name, params.lines).await {
            Ok(logs) => Ok(Json(MessageResponse::success(logs))),
            Err(Error::ProcessNotFound(_)) => Err(StatusCode::NOT_FOUND),
            Err(e) => {
                eprintln!("Error getting process logs: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}
