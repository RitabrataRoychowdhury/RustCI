use crate::ci::config::CIPipeline;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineExecution {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub mongo_id: Option<ObjectId>,
    pub id: Uuid,
    pub pipeline_id: Uuid,
    pub status: ExecutionStatus,
    pub trigger_info: TriggerInfo,
    pub stages: Vec<StageExecution>,
    pub environment: HashMap<String, String>,
    pub logs: Vec<LogEntry>,
    pub artifacts: Vec<Artifact>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Option<u64>, // in seconds
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerInfo {
    pub trigger_type: String,
    pub triggered_by: Option<String>,
    pub commit_hash: Option<String>,
    pub branch: Option<String>,
    pub repository: Option<String>,
    pub webhook_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageExecution {
    pub name: String,
    pub status: ExecutionStatus,
    pub steps: Vec<StepExecution>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecution {
    pub name: String,
    pub status: ExecutionStatus,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: LogLevel,
    pub stage: Option<String>,
    pub step: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub content_type: String,
    pub checksum: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl PipelineExecution {
    pub fn new(pipeline_id: Uuid, trigger_info: TriggerInfo) -> Self {
        Self {
            mongo_id: None,
            id: Uuid::new_v4(),
            pipeline_id,
            status: ExecutionStatus::Pending,
            trigger_info,
            stages: Vec::new(),
            environment: HashMap::new(),
            logs: Vec::new(),
            artifacts: Vec::new(),
            started_at: None,
            finished_at: None,
            duration: None,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        }
    }

    pub fn start(&mut self) {
        self.status = ExecutionStatus::Running;
        self.started_at = Some(chrono::Utc::now());
        self.updated_at = Some(chrono::Utc::now());
    }

    pub fn finish(&mut self, status: ExecutionStatus) {
        self.status = status;
        self.finished_at = Some(chrono::Utc::now());
        self.updated_at = Some(chrono::Utc::now());

        if let (Some(started), Some(finished)) = (self.started_at, self.finished_at) {
            self.duration = Some((finished - started).num_seconds() as u64);
        }
    }

    pub fn add_log(
        &mut self,
        level: LogLevel,
        message: String,
        stage: Option<String>,
        step: Option<String>,
    ) {
        self.logs.push(LogEntry {
            timestamp: chrono::Utc::now(),
            level,
            stage,
            step,
            message,
        });
        self.updated_at = Some(chrono::Utc::now());
    }

    #[allow(dead_code)] // 👈 Add this line
    pub fn add_artifact(&mut self, artifact: Artifact) {
        self.artifacts.push(artifact);
        self.updated_at = Some(chrono::Utc::now());
    }

    pub fn initialize_stages(&mut self, pipeline: &CIPipeline) {
        self.stages = pipeline
            .stages
            .iter()
            .map(|stage| StageExecution {
                name: stage.name.clone(),
                status: ExecutionStatus::Pending,
                steps: stage
                    .steps
                    .iter()
                    .map(|step| StepExecution {
                        name: step.name.clone(),
                        status: ExecutionStatus::Pending,
                        command: None,
                        exit_code: None,
                        stdout: None,
                        stderr: None,
                        started_at: None,
                        finished_at: None,
                        duration: None,
                    })
                    .collect(),
                started_at: None,
                finished_at: None,
                duration: None,
            })
            .collect();
    }

    pub fn get_stage_mut(&mut self, stage_name: &str) -> Option<&mut StageExecution> {
        self.stages.iter_mut().find(|s| s.name == stage_name)
    }

    pub fn get_step_mut(
        &mut self,
        stage_name: &str,
        step_name: &str,
    ) -> Option<&mut StepExecution> {
        self.get_stage_mut(stage_name)?
            .steps
            .iter_mut()
            .find(|s| s.name == step_name)
    }
}

impl StageExecution {
    pub fn start(&mut self) {
        self.status = ExecutionStatus::Running;
        self.started_at = Some(chrono::Utc::now());
    }

    pub fn finish(&mut self, status: ExecutionStatus) {
        self.status = status;
        self.finished_at = Some(chrono::Utc::now());

        if let (Some(started), Some(finished)) = (self.started_at, self.finished_at) {
            self.duration = Some((finished - started).num_seconds() as u64);
        }
    }
}

// (your existing code remains unchanged up to the bottom)

impl StepExecution {
    pub fn start(&mut self, command: Option<String>) {
        self.status = ExecutionStatus::Running;
        self.command = command;
        self.started_at = Some(chrono::Utc::now());
    }

    pub fn finish(
        &mut self,
        status: ExecutionStatus,
        exit_code: Option<i32>,
        stdout: Option<String>,
        stderr: Option<String>,
    ) {
        self.status = status;
        self.exit_code = exit_code;
        self.stdout = stdout;
        self.stderr = stderr;
        self.finished_at = Some(chrono::Utc::now());

        if let (Some(started), Some(finished)) = (self.started_at, self.finished_at) {
            self.duration = Some((finished - started).num_seconds() as u64);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// ✅ Added below this line: Unit test to make sure `add_artifact` is used
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_add_artifact() {
        let trigger_info = TriggerInfo {
            trigger_type: "manual".into(),
            triggered_by: Some("user123".into()),
            commit_hash: Some("abc123".into()),
            branch: Some("main".into()),
            repository: Some("repo".into()),
            webhook_payload: None,
        };

        let mut pipeline = PipelineExecution::new(Uuid::new_v4(), trigger_info);

        let artifact = Artifact {
            name: "build.tar.gz".into(),
            path: "/tmp/build.tar.gz".into(),
            size: 1024,
            content_type: "application/gzip".into(),
            checksum: "dummychecksum".into(),
            created_at: Utc::now(),
        };

        pipeline.add_artifact(artifact.clone());

        assert_eq!(pipeline.artifacts.len(), 1);
        assert_eq!(pipeline.artifacts[0].name, artifact.name);
    }
}
