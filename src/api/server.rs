#[cfg(feature = "http-api")]
use crate::{
    api::{auth::AuthManager, handlers::*, docs::ApiDoc},
    process::ProcessManager,
    Error, Result,
};
#[cfg(feature = "http-api")]
use axum::{
    routing::{delete, get, post, put},
    Router,
};
#[cfg(feature = "http-api")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "http-api")]
use tower::ServiceBuilder;
#[cfg(feature = "http-api")]
use tower_http::{cors::CorsLayer, trace::TraceLayer};
#[cfg(feature = "http-api")]
use utoipa_swagger_ui::SwaggerUi;

#[cfg(feature = "http-api")]
pub struct ApiServer {
    process_manager: Arc<ProcessManager>,
    auth_manager: Arc<Mutex<AuthManager>>,
    port: u16,
}

#[cfg(feature = "http-api")]
impl ApiServer {
    pub fn new(process_manager: ProcessManager, port: u16) -> Result<Self> {
        let auth_manager = AuthManager::new()?;
        Ok(Self {
            process_manager: Arc::new(process_manager),
            auth_manager: Arc::new(Mutex::new(auth_manager)),
            port,
        })
    }

    pub fn get_auth_manager(&self) -> Arc<Mutex<AuthManager>> {
        self.auth_manager.clone()
    }

    pub async fn start(&self) -> Result<()> {
        let app = self.create_router();

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .map_err(|e| Error::Other(format!("Failed to bind to port {}: {}", self.port, e)))?;

        println!("PMR HTTP API server starting on port {}", self.port);
        println!("API endpoints:");
        println!("  GET    /api/processes           - List all processes");
        println!("  POST   /api/processes           - Start a new process");
        println!("  GET    /api/processes/{{name}}   - Get process status");
        println!("  PUT    /api/processes/{{name}}/stop    - Stop a process");
        println!("  PUT    /api/processes/{{name}}/restart - Restart a process");
        println!("  DELETE /api/processes/{{name}}   - Delete a process");
        println!("  GET    /api/processes/{{name}}/logs    - Get process logs");
        println!();
        println!("API Documentation:");
        println!("  Swagger UI: http://localhost:{}/swagger-ui/", self.port);
        println!("  OpenAPI JSON: http://localhost:{}/api-docs/openapi.json", self.port);

        axum::serve(listener, app)
            .await
            .map_err(|e| Error::Other(format!("Server error: {}", e)))?;

        Ok(())
    }

    fn create_router(&self) -> Router {
        let api_routes = Router::new()
            .route("/processes", get(list_processes))
            .route("/processes", post(start_process))
            .route("/processes/:name", get(get_process_status))
            .route("/processes/:name/stop", put(stop_process))
            .route("/processes/:name/restart", put(restart_process))
            .route("/processes/:name", delete(delete_process))
            .route("/processes/:name/logs", get(get_process_logs))
            .with_state((self.process_manager.clone(), self.auth_manager.clone()));

        Router::new()
            .nest("/api", api_routes)
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::get_openapi()))
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive()),
            )
    }
}


