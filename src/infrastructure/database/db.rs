use mongodb::{
    bson::{doc, oid::ObjectId, DateTime as BsonDateTime},
    options::{ClientOptions, ServerApi, ServerApiVersion},
    Client, Collection, Database,
};
use tracing::info;

use crate::{domain::entities::User, error::AppError};

#[derive(Clone)]
pub struct DatabaseManager {
    pub client: Client,
    pub database: Database,
}

impl DatabaseManager {
    pub async fn new(mongodb_uri: &str, database_name: &str) -> Result<Self, AppError> {
        info!("🔄 Connecting to MongoDB...");

        let mut client_options = ClientOptions::parse(mongodb_uri)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to parse MongoDB URI: {}", e)))?;

        // Set the server_api field of the client_options object to Stable API version 1
        let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
        client_options.server_api = Some(server_api);

        // Create a new client and connect to the server
        let client = Client::with_options(client_options).map_err(|e| {
            AppError::DatabaseError(format!("Failed to create MongoDB client: {}", e))
        })?;

        // Send a ping to confirm a successful connection
        client
            .database("admin")
            .run_command(doc! {"ping": 1}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to ping MongoDB: {}", e)))?;

        info!("✅ Successfully connected to MongoDB!");

        let database = client.database(database_name);

        Ok(DatabaseManager { client, database })
    }

    pub fn get_users_collection(&self) -> Collection<User> {
        self.database.collection::<User>("users")
    }

    pub fn get_database(&self) -> &Database {
        &self.database
    }

    pub async fn create_user(&self, user: &User) -> Result<String, AppError> {
        let collection = self.get_users_collection();

        let result = collection
            .insert_one(user, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to create user: {}", e)))?;

        match result.inserted_id.as_object_id() {
            Some(oid) => Ok(oid.to_hex()),
            None => Err(AppError::DatabaseError(
                "Failed to get inserted user ID".to_string(),
            )),
        }
    }

    pub async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
        let collection = self.get_users_collection();

        let user = collection
            .find_one(doc! {"email": email}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find user by email: {}", e)))?;

        Ok(user)
    }

    pub async fn find_user_by_id(&self, user_id: &str) -> Result<Option<User>, AppError> {
        let collection = self.get_users_collection();

        let object_id = ObjectId::parse_str(user_id)
            .map_err(|e| AppError::DatabaseError(format!("Invalid user ID format: {}", e)))?;

        let user = collection
            .find_one(doc! {"_id": object_id}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find user by ID: {}", e)))?;

        Ok(user)
    }

    pub async fn update_user(&self, user_id: &str, user: &User) -> Result<(), AppError> {
        let collection = self.get_users_collection();

        let object_id = ObjectId::parse_str(user_id)
            .map_err(|e| AppError::DatabaseError(format!("Invalid user ID format: {}", e)))?;

        let update_doc = doc! {
            "$set": {
                "name": &user.name,
                "email": &user.email,
                "photo": &user.photo,
                "verified": user.verified,
                "provider": &user.provider,
                "role": &user.role,
                "updated_at": BsonDateTime::from_system_time(chrono::Utc::now().into())
            }
        };

        collection
            .update_one(doc! {"_id": object_id}, update_doc, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to update user: {}", e)))?;

        Ok(())
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<(), AppError> {
        let collection = self.get_users_collection();

        let object_id = ObjectId::parse_str(user_id)
            .map_err(|e| AppError::DatabaseError(format!("Invalid user ID format: {}", e)))?;

        collection
            .delete_one(doc! {"_id": object_id}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to delete user: {}", e)))?;

        Ok(())
    }

    pub async fn find_or_create_oauth_user(
        &self,
        github_user: &crate::application::handlers::auth::GitHubUserResult,
    ) -> Result<User, AppError> {
        // Create owned email string, using fallback if needed
        let email = github_user
            .email
            .clone()
            .unwrap_or_else(|| format!("{}@github.local", github_user.login));

        // Try to find existing user
        if let Some(existing_user) = self.find_user_by_email(&email).await? {
            info!("✅ Found existing user: {}", existing_user.email);
            return Ok(existing_user);
        }

        // Create new user
        let new_user = User {
            mongo_id: None,
            id: uuid::Uuid::new_v4(),
            email: email.clone(),
            name: github_user
                .name
                .clone()
                .unwrap_or_else(|| github_user.login.clone()),
            photo: github_user.avatar_url.clone(),
            verified: true,
            provider: "GitHub".to_string(),
            role: "user".to_string(),
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        };

        self.create_user(&new_user).await?;
        info!("✅ Created new user: {}", new_user.email);

        Ok(new_user)
    }
}
