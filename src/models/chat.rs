use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct Chat {
    pub id: Uuid,
    pub chat_type: Option<String>,               // 'direct' or 'group'
    pub user_a: Option<Uuid>,         // nullable for group chats
    pub user_b: Option<Uuid>,         // nullable for group chats
    pub name: Option<String>,         // for group chats
    pub owner_id: Option<Uuid>,       // for group chats
    pub last_message_id: Option<Uuid>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}
#[derive(Debug, Serialize, Deserialize)]

pub struct ChatView {
    pub id: Uuid,
    pub chat_type: Option<String>,
    pub partner_user_id: Option<Uuid>,
    pub partner_username: Option<String>,
    pub name: Option<String>,
    pub last_message_id: Option<Uuid>,
    pub updated_at: Option<NaiveDateTime>,
}