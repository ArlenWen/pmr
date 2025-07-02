#[cfg(feature = "http-api")]
use utoipa::OpenApi;

#[cfg(feature = "http-api")]
use crate::{
    api::handlers::{ApiResponse, StartProcessRequest, LogsQuery},
    database::{ProcessRecord, ProcessStatus},
};

#[cfg(feature = "http-api")]
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::handlers::list_processes,
        crate::api::handlers::get_process_status,
        crate::api::handlers::start_process,
        crate::api::handlers::stop_process,
        crate::api::handlers::restart_process,
        crate::api::handlers::delete_process,
        crate::api::handlers::get_process_logs,
    ),
    components(
        schemas(
            ProcessRecord,
            ProcessStatus,
            ApiResponse<Vec<ProcessRecord>>,
            ApiResponse<ProcessRecord>,
            ApiResponse<String>,
            StartProcessRequest,
            LogsQuery,
        )
    ),
    tags(
        (name = "processes", description = "Process management operations")
    ),
    info(
        title = "PMR API",
        description = "Process Management Tool REST API",
        version = "0.2.0",
        contact(
            name = "PMR",
            url = "https://github.com/ArlenWen/pmr"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    servers(
        (url = "http://localhost:8080", description = "Local development server")
    ),
    security(
        ("bearer_auth" = ["ApiKey"])
    )
)]
pub struct ApiDoc;

#[cfg(feature = "http-api")]
impl ApiDoc {
    pub fn get_openapi() -> utoipa::openapi::OpenApi {
        let mut openapi = <Self as utoipa::OpenApi>::openapi();

        // Add security scheme
        if let Some(components) = openapi.components.as_mut() {
            use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build()
                )
            );
        }

        openapi
    }
}
