mod chat;
mod chat_file;
mod message;
mod user;
mod workspace;

use serde::{Deserialize, Serialize};

pub use chat::{ChatRepo, CreateChat};
pub use message::{CreateMessage, ListMessages, MessageRepo};
pub use user::{CreateUser, SigninUser, UserRepo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatFile {
    pub ws_id: u64,
    pub ext: String, // extract ext from filename or mime type
    pub hash: String,
}
