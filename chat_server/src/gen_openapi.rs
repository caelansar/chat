use chat_server::ApiDoc;
use std::fs;
use utoipa::OpenApi;

fn main() {
    let doc = ApiDoc::openapi().to_pretty_json().unwrap();
    fs::write("chat_server/api-docs/openapi.json", doc).unwrap();
}
