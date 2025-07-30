//! Runner repository interfaces and implementations
//!
//! This module defines the repository interfaces for managing runners and jobs
//! in the RustCI platform's persistence layer.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::entities::{
    Job, JobId, JobResult, JobStatus, NodeId, PipelineId, RunnerEntity, RunnerId, RunnerStatus,
};
use crate::error::Result;

/// Repository interface for managing runners
#[async_trait]
pub trait RunnerRepository: Send + Sync {
    /// Create a new runner
    async fn create(&self, runner: &RunnerEntity) -> Result<RunnerEntity>;

    /// Find a runner by its ID
    async fn find_by_id(&self, runner_id: RunnerId) -> Result<Option<RunnerEntity>>;

    /// Find all runners
    async fn find_all(&self) -> Result<Vec<RunnerEntity>>;

    /// Find runners by status
    async fn find_by_status(&self, status: RunnerStatus) -> Result<Vec<RunnerEntity>>;

    /// Find runners by node ID
    async fn find_by_node_id(&self, node_id: NodeId) -> Result<Vec<RunnerEntity>>;

    /// Find runners by tags
    async fn find_by_tags(&self, tags: Vec<String>) -> Result<Vec<RunnerEntity>>;

    /// Update a runner
    async fn update(&self, runner: &RunnerEntity) -> Result<RunnerEntity>;

    /// Update runner status
    async fn update_status(&self, runner_id: RunnerId, status: RunnerStatus) -> Result<()>;

    /// Update runner heartbeat
    async fn update_heartbeat(&self, runner_id: RunnerId, timestamp: DateTime<Utc>) -> Result<()>;

    /// Delete a runner
    async fn delete(&self, runner_id: RunnerId) -> Result<()>;

    /// Check if a runner exists
    async fn exists(&self, runner_id: RunnerId) -> Result<bool>;

    /// Count total runners
    async fn count(&self) -> Result<u64>;

    /// Count runners by status
    async fn count_by_status(&self, status: RunnerStatus) -> Result<u64>;

    /// Find available runners (active or idle)
    async fn find_available(&self) -> Result<Vec<RunnerEntity>>;

    /// Find runners with pagination
    async fn find_with_pagination(&self, limit: usize, offset: usize) -> Result<Vec<RunnerEntity>>;
}

/// Repository interface for managing jobs
#[async_trait]
pub trait JobRepository: Send + Sync {
    /// Create a new job
    async fn create(&self, job: &Job) -> Result<Job>;

    /// Find a job by its ID
    async fn find_by_id(&self, job_id: JobId) -> Result<Option<Job>>;

    /// Find all jobs
    async fn find_all(&self) -> Result<Vec<Job>>;

    /// Find jobs by status
    async fn find_by_status(&self, status: JobStatus) -> Result<Vec<Job>>;

    /// Find jobs by pipeline ID
    async fn find_by_pipeline_id(&self, pipeline_id: PipelineId) -> Result<Vec<Job>>;

    /// Find jobs by runner ID
    async fn find_by_runner_id(&self, runner_id: RunnerId) -> Result<Vec<Job>>;

    /// Find queued jobs
    async fn find_queued(&self) -> Result<Vec<Job>>;

    /// Find running jobs
    async fn find_running(&self) -> Result<Vec<Job>>;

    /// Find jobs by priority
    async fn find_by_priority(
        &self,
        priority: crate::domain::entities::JobPriority,
    ) -> Result<Vec<Job>>;

    /// Find jobs created within a time range
    async fn find_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Job>>;

    /// Update a job
    async fn update(&self, job: &Job) -> Result<Job>;

    /// Update job status
    async fn update_status(&self, job_id: JobId, status: JobStatus) -> Result<()>;

    /// Delete a job
    async fn delete(&self, job_id: JobId) -> Result<()>;

    /// Check if a job exists
    async fn exists(&self, job_id: JobId) -> Result<bool>;

    /// Count total jobs
    async fn count(&self) -> Result<u64>;

    /// Count jobs by status
    async fn count_by_status(&self, status: JobStatus) -> Result<u64>;

    /// Find jobs with pagination
    async fn find_with_pagination(&self, limit: usize, offset: usize) -> Result<Vec<Job>>;

    /// Find next job to execute (highest priority, oldest first)
    async fn find_next_job(&self) -> Result<Option<Job>>;

    /// Find jobs that match runner capabilities
    async fn find_matching_jobs(&self, runner: &RunnerEntity) -> Result<Vec<Job>>;
}

/// Repository interface for managing job results
#[async_trait]
pub trait JobResultRepository: Send + Sync {
    /// Create a new job result
    async fn create(&self, result: &JobResult) -> Result<JobResult>;

    /// Find a job result by job ID
    async fn find_by_job_id(&self, job_id: JobId) -> Result<Option<JobResult>>;

    /// Find all job results
    async fn find_all(&self) -> Result<Vec<JobResult>>;

    /// Find job results by status
    async fn find_by_status(&self, status: JobStatus) -> Result<Vec<JobResult>>;

    /// Find job results by pipeline ID
    async fn find_by_pipeline_id(&self, pipeline_id: PipelineId) -> Result<Vec<JobResult>>;

    /// Find job results within a time range
    async fn find_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<JobResult>>;

    /// Update a job result
    async fn update(&self, result: &JobResult) -> Result<JobResult>;

    /// Delete a job result
    async fn delete(&self, job_id: JobId) -> Result<()>;

    /// Check if a job result exists
    async fn exists(&self, job_id: JobId) -> Result<bool>;

    /// Count total job results
    async fn count(&self) -> Result<u64>;

    /// Count job results by status
    async fn count_by_status(&self, status: JobStatus) -> Result<u64>;

    /// Find job results with pagination
    async fn find_with_pagination(&self, limit: usize, offset: usize) -> Result<Vec<JobResult>>;

    /// Get job execution statistics
    async fn get_execution_stats(&self) -> Result<JobExecutionStats>;

    /// Get job results for a specific time period
    async fn get_results_by_period(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<JobResult>>;
}

/// Job execution statistics
#[derive(Debug, Clone)]
pub struct JobExecutionStats {
    /// Total number of jobs executed
    pub total_jobs: u64,
    /// Number of successful jobs
    pub successful_jobs: u64,
    /// Number of failed jobs
    pub failed_jobs: u64,
    /// Number of cancelled jobs
    pub cancelled_jobs: u64,
    /// Number of timed out jobs
    pub timed_out_jobs: u64,
    /// Average execution time in seconds
    pub avg_execution_time: f64,
    /// Success rate as percentage
    pub success_rate: f64,
    /// Jobs per hour throughput
    pub jobs_per_hour: f64,
}

impl JobExecutionStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self {
            total_jobs: 0,
            successful_jobs: 0,
            failed_jobs: 0,
            cancelled_jobs: 0,
            timed_out_jobs: 0,
            avg_execution_time: 0.0,
            success_rate: 0.0,
            jobs_per_hour: 0.0,
        }
    }

    /// Calculate success rate
    pub fn calculate_success_rate(&mut self) {
        if self.total_jobs > 0 {
            self.success_rate = (self.successful_jobs as f64 / self.total_jobs as f64) * 100.0;
        }
    }
}

impl Default for JobExecutionStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Query parameters for finding jobs
#[derive(Debug, Clone, Default)]
pub struct JobQuery {
    /// Filter by status
    pub status: Option<JobStatus>,
    /// Filter by pipeline ID
    pub pipeline_id: Option<PipelineId>,
    /// Filter by runner ID
    pub runner_id: Option<RunnerId>,
    /// Filter by priority
    pub priority: Option<crate::domain::entities::JobPriority>,
    /// Filter by creation time range
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    /// Filter by tags
    pub required_tags: Vec<String>,
    pub excluded_tags: Vec<String>,
    /// Pagination
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// Sorting
    pub sort_by: Option<JobSortField>,
    pub sort_order: Option<SortOrder>,
}

/// Fields to sort jobs by
#[derive(Debug, Clone)]
pub enum JobSortField {
    CreatedAt,
    Priority,
    Status,
    Name,
}

/// Sort order
#[derive(Debug, Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Query parameters for finding runners
#[derive(Debug, Clone, Default)]
pub struct RunnerQuery {
    /// Filter by status
    pub status: Option<RunnerStatus>,
    /// Filter by node ID
    pub node_id: Option<NodeId>,
    /// Filter by tags
    pub required_tags: Vec<String>,
    pub excluded_tags: Vec<String>,
    /// Filter by runner type
    pub runner_type: Option<String>,
    /// Filter by availability
    pub available_only: bool,
    /// Pagination
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// Sorting
    pub sort_by: Option<RunnerSortField>,
    pub sort_order: Option<SortOrder>,
}

/// Fields to sort runners by
#[derive(Debug, Clone)]
pub enum RunnerSortField {
    Name,
    Status,
    CreatedAt,
    LastHeartbeat,
}




#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_execution_stats_creation() {
        let stats = JobExecutionStats::new();
        assert_eq!(stats.total_jobs, 0);
        assert_eq!(stats.successful_jobs, 0);
        assert_eq!(stats.success_rate, 0.0);
    }

    #[test]
    fn test_job_execution_stats_success_rate() {
        let mut stats = JobExecutionStats::new();
        stats.total_jobs = 100;
        stats.successful_jobs = 85;
        stats.calculate_success_rate();
        assert_eq!(stats.success_rate, 85.0);
    }

    #[test]
    fn test_job_query_default() {
        let query = JobQuery::default();
        assert!(query.status.is_none());
        assert!(query.pipeline_id.is_none());
        assert_eq!(query.required_tags.len(), 0);
    }

    #[test]
    fn test_runner_query_default() {
        let query = RunnerQuery::default();
        assert!(query.status.is_none());
        assert!(query.node_id.is_none());
        assert!(!query.available_only);
    }
}
