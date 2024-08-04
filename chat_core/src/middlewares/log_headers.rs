use axum::{extract::Request, middleware::Next, response::Response};
use tracing::info;

pub async fn log_headers(req: Request, next: Next) -> Response {
    let headers = req.headers();
    info!("url: {}, header: {:?}", req.uri(), headers);

    next.run(req).await
}
