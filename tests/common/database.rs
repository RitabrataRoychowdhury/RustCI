use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Test database implementation for isolated testing
pub struct TestDatabase {
    collections: Arc<RwLock<HashMap<String, Vec<TestDocument>>>>,
    transactions: Arc<RwLock<Vec<TestTransaction>>>,
    should_fail: Arc<RwLock<bool>>,
}

impl TestDatabase {
    pub async fn new() -> Self {
        Self {
            collections: Arc::new(RwLock::new(HashMap::new())),
            transactions: Arc::new(RwLock::new(Vec::new())),
            should_fail: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn insert(
        &self,
        collection: &str,
        document: TestDocument,
    ) -> Result<Uuid, TestDbError> {
        if *self.should_fail.read().await {
            return Err(TestDbError::ConnectionFailed);
        }

        let mut collections = self.collections.write().await;
        let docs = collections
            .entry(collection.to_string())
            .or_insert_with(Vec::new);
        docs.push(document.clone());
        Ok(document.id)
    }

    pub async fn find_by_id(
        &self,
        collection: &str,
        id: Uuid,
    ) -> Result<Option<TestDocument>, TestDbError> {
        if *self.should_fail.read().await {
            return Err(TestDbError::ConnectionFailed);
        }

        let collections = self.collections.read().await;
        if let Some(docs) = collections.get(collection) {
            Ok(docs.iter().find(|doc| doc.id == id).cloned())
        } else {
            Ok(None)
        }
    }

    pub async fn find_all(&self, collection: &str) -> Result<Vec<TestDocument>, TestDbError> {
        if *self.should_fail.read().await {
            return Err(TestDbError::ConnectionFailed);
        }

        let collections = self.collections.read().await;
        Ok(collections.get(collection).cloned().unwrap_or_default())
    }

    pub async fn update(
        &self,
        collection: &str,
        id: Uuid,
        updates: Value,
    ) -> Result<bool, TestDbError> {
        if *self.should_fail.read().await {
            return Err(TestDbError::ConnectionFailed);
        }

        let mut collections = self.collections.write().await;
        if let Some(docs) = collections.get_mut(collection) {
            if let Some(doc) = docs.iter_mut().find(|doc| doc.id == id) {
                if let Value::Object(update_map) = updates {
                    if let Value::Object(ref mut data_map) = doc.data {
                        for (key, value) in update_map {
                            data_map.insert(key, value);
                        }
                    }
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn delete(&self, collection: &str, id: Uuid) -> Result<bool, TestDbError> {
        if *self.should_fail.read().await {
            return Err(TestDbError::ConnectionFailed);
        }

        let mut collections = self.collections.write().await;
        if let Some(docs) = collections.get_mut(collection) {
            if let Some(pos) = docs.iter().position(|doc| doc.id == id) {
                docs.remove(pos);
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn count(&self, collection: &str) -> Result<usize, TestDbError> {
        if *self.should_fail.read().await {
            return Err(TestDbError::ConnectionFailed);
        }

        let collections = self.collections.read().await;
        Ok(collections
            .get(collection)
            .map(|docs| docs.len())
            .unwrap_or(0))
    }

    pub async fn clear_collection(&self, collection: &str) {
        let mut collections = self.collections.write().await;
        collections.remove(collection);
    }

    pub async fn begin_transaction(&self) -> TestTransaction {
        let transaction = TestTransaction {
            id: Uuid::new_v4(),
            operations: Vec::new(),
            committed: false,
            rolled_back: false,
        };

        self.transactions.write().await.push(transaction.clone());
        transaction
    }

    pub async fn commit_transaction(&self, transaction_id: Uuid) -> Result<(), TestDbError> {
        if *self.should_fail.read().await {
            return Err(TestDbError::TransactionFailed);
        }

        let mut transactions = self.transactions.write().await;
        if let Some(transaction) = transactions.iter_mut().find(|t| t.id == transaction_id) {
            transaction.committed = true;
            Ok(())
        } else {
            Err(TestDbError::TransactionNotFound)
        }
    }

    pub async fn rollback_transaction(&self, transaction_id: Uuid) -> Result<(), TestDbError> {
        let mut transactions = self.transactions.write().await;
        if let Some(transaction) = transactions.iter_mut().find(|t| t.id == transaction_id) {
            transaction.rolled_back = true;
            Ok(())
        } else {
            Err(TestDbError::TransactionNotFound)
        }
    }

    pub async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.write().await = should_fail;
    }

    pub async fn cleanup(&self) {
        self.collections.write().await.clear();
        self.transactions.write().await.clear();
        *self.should_fail.write().await = false;
    }

    pub async fn get_collection_names(&self) -> Vec<String> {
        self.collections.read().await.keys().cloned().collect()
    }

    pub async fn create_index(
        &self,
        _collection: &str,
        _index: TestIndex,
    ) -> Result<(), TestDbError> {
        // Mock implementation - just return success
        Ok(())
    }

    pub async fn query(
        &self,
        collection: &str,
        filter: TestFilter,
    ) -> Result<Vec<TestDocument>, TestDbError> {
        if *self.should_fail.read().await {
            return Err(TestDbError::ConnectionFailed);
        }

        let collections = self.collections.read().await;
        if let Some(docs) = collections.get(collection) {
            let filtered: Vec<TestDocument> = docs
                .iter()
                .filter(|doc| filter.matches(doc))
                .cloned()
                .collect();
            Ok(filtered)
        } else {
            Ok(Vec::new())
        }
    }
}

/// Test document structure
#[derive(Debug, Clone)]
pub struct TestDocument {
    pub id: Uuid,
    pub data: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl TestDocument {
    pub fn new(data: Value) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            data,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }
}

/// Test transaction
#[derive(Debug, Clone)]
pub struct TestTransaction {
    pub id: Uuid,
    pub operations: Vec<TestOperation>,
    pub committed: bool,
    pub rolled_back: bool,
}

/// Test database operation
#[derive(Debug, Clone)]
pub enum TestOperation {
    Insert {
        collection: String,
        document: TestDocument,
    },
    Update {
        collection: String,
        id: Uuid,
        updates: Value,
    },
    Delete {
        collection: String,
        id: Uuid,
    },
}

/// Test database index
#[derive(Debug, Clone)]
pub struct TestIndex {
    pub name: String,
    pub fields: Vec<String>,
    pub unique: bool,
}

/// Test query filter
#[derive(Debug, Clone)]
pub struct TestFilter {
    pub conditions: HashMap<String, FilterCondition>,
}

impl TestFilter {
    pub fn new() -> Self {
        Self {
            conditions: HashMap::new(),
        }
    }

    pub fn equals(mut self, field: &str, value: Value) -> Self {
        self.conditions
            .insert(field.to_string(), FilterCondition::Equals(value));
        self
    }

    pub fn contains(mut self, field: &str, value: String) -> Self {
        self.conditions
            .insert(field.to_string(), FilterCondition::Contains(value));
        self
    }

    pub fn matches(&self, document: &TestDocument) -> bool {
        for (field, condition) in &self.conditions {
            if !condition.matches(&document.data, field) {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub enum FilterCondition {
    Equals(Value),
    Contains(String),
    GreaterThan(Value),
    LessThan(Value),
}

impl FilterCondition {
    pub fn matches(&self, data: &Value, field: &str) -> bool {
        let field_value = data.get(field);

        match (self, field_value) {
            (FilterCondition::Equals(expected), Some(actual)) => expected == actual,
            (FilterCondition::Contains(substring), Some(Value::String(text))) => {
                text.contains(substring)
            }
            _ => false,
        }
    }
}

/// Test database errors
#[derive(Debug, thiserror::Error)]
pub enum TestDbError {
    #[error("Connection failed")]
    ConnectionFailed,

    #[error("Transaction failed")]
    TransactionFailed,

    #[error("Transaction not found")]
    TransactionNotFound,

    #[error("Collection not found: {0}")]
    CollectionNotFound(String),

    #[error("Document not found")]
    DocumentNotFound,

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Index creation failed: {0}")]
    IndexCreationFailed(String),
}

/// Database test utilities
pub struct DatabaseTestUtils;

impl DatabaseTestUtils {
    pub async fn seed_test_data(db: &TestDatabase) -> Result<(), TestDbError> {
        // Seed users
        let user_data = serde_json::json!({
            "username": "testuser",
            "email": "test@example.com",
            "github_id": "123456",
            "roles": ["user"]
        });
        db.insert("users", TestDocument::new(user_data)).await?;

        // Seed workspaces
        let workspace_data = serde_json::json!({
            "name": "test-workspace",
            "owner_id": "550e8400-e29b-41d4-a716-446655440000",
            "repository_url": "https://github.com/test/repo",
            "branch": "main",
            "status": "active"
        });
        db.insert("workspaces", TestDocument::new(workspace_data))
            .await?;

        // Seed pipelines
        let pipeline_data = serde_json::json!({
            "name": "test-pipeline",
            "stages": [
                {
                    "name": "build",
                    "steps": [
                        {
                            "name": "compile",
                            "command": "cargo build",
                            "args": ["--release"]
                        }
                    ]
                }
            ],
            "triggers": ["push"]
        });
        db.insert("pipelines", TestDocument::new(pipeline_data))
            .await?;

        Ok(())
    }

    pub async fn create_test_indexes(db: &TestDatabase) -> Result<(), TestDbError> {
        // Create indexes for common queries
        db.create_index(
            "users",
            TestIndex {
                name: "username_idx".to_string(),
                fields: vec!["username".to_string()],
                unique: true,
            },
        )
        .await?;

        db.create_index(
            "workspaces",
            TestIndex {
                name: "owner_idx".to_string(),
                fields: vec!["owner_id".to_string()],
                unique: false,
            },
        )
        .await?;

        db.create_index(
            "pipelines",
            TestIndex {
                name: "name_idx".to_string(),
                fields: vec!["name".to_string()],
                unique: false,
            },
        )
        .await?;

        Ok(())
    }
}
