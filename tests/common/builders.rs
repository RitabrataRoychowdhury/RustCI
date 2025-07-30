use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// Builder pattern for test data creation
pub struct TestDataBuilder;

impl TestDataBuilder {
    pub fn pipeline() -> PipelineTestBuilder {
        PipelineTestBuilder::new()
    }

    pub fn user() -> UserTestBuilder {
        UserTestBuilder::new()
    }

    pub fn workspace() -> WorkspaceTestBuilder {
        WorkspaceTestBuilder::new()
    }

    pub fn execution_context() -> ExecutionContextBuilder {
        ExecutionContextBuilder::new()
    }

    pub fn command() -> CommandTestBuilder {
        CommandTestBuilder::new()
    }

    pub fn event() -> EventTestBuilder {
        EventTestBuilder::new()
    }
}

/// Pipeline test builder
pub struct PipelineTestBuilder {
    id: Uuid,
    name: String,
    stages: Vec<TestStage>,
    variables: HashMap<String, String>,
    triggers: Vec<String>,
}

impl PipelineTestBuilder {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "test-pipeline".to_string(),
            stages: vec![],
            variables: HashMap::new(),
            triggers: vec!["push".to_string()],
        }
    }

    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn with_stage(mut self, stage: TestStage) -> Self {
        self.stages.push(stage);
        self
    }

    pub fn with_variable(mut self, key: &str, value: &str) -> Self {
        self.variables.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_trigger(mut self, trigger: &str) -> Self {
        self.triggers.push(trigger.to_string());
        self
    }

    pub fn build(self) -> TestPipeline {
        TestPipeline {
            id: self.id,
            name: self.name,
            stages: self.stages,
            variables: self.variables,
            triggers: self.triggers,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// User test builder
pub struct UserTestBuilder {
    id: Uuid,
    username: String,
    email: String,
    github_id: Option<String>,
    roles: Vec<String>,
}

impl UserTestBuilder {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            github_id: Some("123456".to_string()),
            roles: vec!["user".to_string()],
        }
    }

    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    pub fn with_username(mut self, username: &str) -> Self {
        self.username = username.to_string();
        self
    }

    pub fn with_email(mut self, email: &str) -> Self {
        self.email = email.to_string();
        self
    }

    pub fn with_github_id(mut self, github_id: &str) -> Self {
        self.github_id = Some(github_id.to_string());
        self
    }

    pub fn with_role(mut self, role: &str) -> Self {
        self.roles.push(role.to_string());
        self
    }

    pub fn admin(mut self) -> Self {
        self.roles.push("admin".to_string());
        self
    }

    pub fn build(self) -> TestUser {
        TestUser {
            id: self.id,
            username: self.username,
            email: self.email,
            github_id: self.github_id,
            roles: self.roles,
            created_at: Utc::now(),
            last_login: None,
        }
    }
}

/// Workspace test builder
pub struct WorkspaceTestBuilder {
    id: Uuid,
    name: String,
    owner_id: Uuid,
    repository_url: Option<String>,
    branch: String,
    status: String,
}

impl WorkspaceTestBuilder {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "test-workspace".to_string(),
            owner_id: Uuid::new_v4(),
            repository_url: Some("https://github.com/test/repo".to_string()),
            branch: "main".to_string(),
            status: "active".to_string(),
        }
    }

    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn with_owner(mut self, owner_id: Uuid) -> Self {
        self.owner_id = owner_id;
        self
    }

    pub fn with_repository(mut self, url: &str) -> Self {
        self.repository_url = Some(url.to_string());
        self
    }

    pub fn with_branch(mut self, branch: &str) -> Self {
        self.branch = branch.to_string();
        self
    }

    pub fn build(self) -> TestWorkspace {
        TestWorkspace {
            id: self.id,
            name: self.name,
            owner_id: self.owner_id,
            repository_url: self.repository_url,
            branch: self.branch,
            status: self.status,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// Execution context builder
pub struct ExecutionContextBuilder {
    pipeline_id: Uuid,
    execution_id: Uuid,
    user_id: Uuid,
    workspace_id: Uuid,
    variables: HashMap<String, String>,
    secrets: HashMap<String, String>,
}

impl ExecutionContextBuilder {
    pub fn new() -> Self {
        Self {
            pipeline_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            variables: HashMap::new(),
            secrets: HashMap::new(),
        }
    }

    pub fn with_pipeline_id(mut self, id: Uuid) -> Self {
        self.pipeline_id = id;
        self
    }

    pub fn with_execution_id(mut self, id: Uuid) -> Self {
        self.execution_id = id;
        self
    }

    pub fn with_variable(mut self, key: &str, value: &str) -> Self {
        self.variables.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_secret(mut self, key: &str, value: &str) -> Self {
        self.secrets.insert(key.to_string(), value.to_string());
        self
    }

    pub fn build(self) -> TestExecutionContext {
        TestExecutionContext {
            pipeline_id: self.pipeline_id,
            execution_id: self.execution_id,
            user_id: self.user_id,
            workspace_id: self.workspace_id,
            variables: self.variables,
            secrets: self.secrets,
            started_at: Utc::now(),
        }
    }
}

/// Command test builder
pub struct CommandTestBuilder {
    id: Uuid,
    command_type: String,
    payload: Value,
    correlation_id: Uuid,
    user_id: Option<Uuid>,
}

impl CommandTestBuilder {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            command_type: "TestCommand".to_string(),
            payload: Value::Object(serde_json::Map::new()),
            correlation_id: Uuid::new_v4(),
            user_id: None,
        }
    }

    pub fn with_type(mut self, command_type: &str) -> Self {
        self.command_type = command_type.to_string();
        self
    }

    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn build(self) -> TestCommand {
        TestCommand {
            id: self.id,
            command_type: self.command_type,
            payload: self.payload,
            correlation_id: self.correlation_id,
            user_id: self.user_id,
            created_at: Utc::now(),
        }
    }
}

/// Event test builder
pub struct EventTestBuilder {
    id: Uuid,
    event_type: String,
    aggregate_id: Uuid,
    data: Value,
    correlation_id: Uuid,
}

impl EventTestBuilder {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: "TestEvent".to_string(),
            aggregate_id: Uuid::new_v4(),
            data: Value::Object(serde_json::Map::new()),
            correlation_id: Uuid::new_v4(),
        }
    }

    pub fn with_type(mut self, event_type: &str) -> Self {
        self.event_type = event_type.to_string();
        self
    }

    pub fn with_aggregate_id(mut self, id: Uuid) -> Self {
        self.aggregate_id = id;
        self
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }

    pub fn build(self) -> TestEvent {
        TestEvent {
            id: self.id,
            event_type: self.event_type,
            aggregate_id: self.aggregate_id,
            data: self.data,
            correlation_id: self.correlation_id,
            occurred_at: Utc::now(),
        }
    }
}

// Test data structures
#[derive(Debug, Clone)]
pub struct TestPipeline {
    pub id: Uuid,
    pub name: String,
    pub stages: Vec<TestStage>,
    pub variables: HashMap<String, String>,
    pub triggers: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TestStage {
    pub name: String,
    pub steps: Vec<TestStep>,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TestStep {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub environment: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct TestUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub github_id: Option<String>,
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct TestWorkspace {
    pub id: Uuid,
    pub name: String,
    pub owner_id: Uuid,
    pub repository_url: Option<String>,
    pub branch: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TestExecutionContext {
    pub pipeline_id: Uuid,
    pub execution_id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub variables: HashMap<String, String>,
    pub secrets: HashMap<String, String>,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TestCommand {
    pub id: Uuid,
    pub command_type: String,
    pub payload: Value,
    pub correlation_id: Uuid,
    pub user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TestEvent {
    pub id: Uuid,
    pub event_type: String,
    pub aggregate_id: Uuid,
    pub data: Value,
    pub correlation_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}
