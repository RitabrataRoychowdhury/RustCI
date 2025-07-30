use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub mongo_id: Option<ObjectId>,
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub photo: String,
    pub verified: bool,
    pub provider: String,
    pub role: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl User {
    pub fn new(name: String, email: String, photo: String, provider: String) -> Self {
        Self {
            mongo_id: None,
            id: Uuid::new_v4(),
            name,
            email,
            photo,
            verified: true,
            provider,
            role: "user".to_string(),
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        }
    }

    pub fn filter_user(&self) -> FilteredUser {
        FilteredUser {
            id: self.id.to_string(),
            name: self.name.clone(),
            email: self.email.clone(),
            photo: self.photo.clone(),
            role: self.role.clone(),
            verified: self.verified,
            provider: self.provider.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilteredUser {
    pub id: String,
    pub name: String,
    pub email: String,
    pub photo: String,
    pub role: String,
    pub verified: bool,
    pub provider: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginUserSchema {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterUserSchema {
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserData {
    pub user: FilteredUser,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub status: String,
    pub data: UserData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserLoginResponse {
    pub status: String,
    pub token: String,
}
