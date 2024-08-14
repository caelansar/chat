use crate::handlers::*;
use crate::ErrorOutput;
use crate::{
    models::{CreateUser, SigninUser},
    AppState,
};
use axum::Router;
use chat_core::{Chat, ChatType};
use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi, ToSchema,
};
use utoipa_rapidoc::RapiDoc;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;

pub(crate) trait OpenApiRouter {
    fn openapi(self) -> Self;
}

#[derive(ToSchema)]
#[allow(unused)]
pub(crate) struct FileForm {
    file: Vec<u8>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        signup_handler,
        signin_handler,
        list_chat_handler,
        create_chat_handler,
        get_chat_handler,
        upload_handler,
    ),
    components(
        schemas(Chat, ChatType, SigninUser, CreateUser, AuthOutput, ErrorOutput, FileForm),
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "chat", description = "Chat related operations"),
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "token",
                SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
            )
        }
    }
}

impl OpenApiRouter for Router<AppState> {
    fn openapi(self) -> Self {
        self.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
            .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"))
    }
}
