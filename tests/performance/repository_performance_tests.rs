use rustci::infrastructure::{
    database::{ProductionDatabaseManager, ProductionDatabaseConfig},
    repositories::{
        EnhancedMongoJobRepository, EnhancedMongoRunnerRepository, 
        EnhancedRunnerRepository, EnhancedRunnerFilter,
    },
};
use rustci::domain::entities::runner::{
    Job, JobPriority, JobStatus, RunnerEntity, RunnerStatus, RunnerType,
    JobStep, JobRequirements, RetryPolicy, BackoffStrategy,
};
use rustci::error::Result;
use chrono::Utc;
use mongodb::Client;
use std::{sync::Arc, time::{Duration, Instant}};
use uuid::Uuid;

/// Create a test job for performance testing
fn create_test_job() -> Job {
    Job {
        id: Uuid::new_v4(),
        pipeline_id: Uuid::new_v4(),
        name: "test-job".to_string(),
        steps: vec![
            JobStep {
                name: "build".to_string(),
                command: "cargo".to_string(),
                args: vec!["build".to_string()],
                working_directory: None,
                environment: std::collections::HashMap::new(),
                timeout: Some(Duration::from_secs(300)),
                continue_on_error: false,
            }
        ],
        requirements: JobRequirements {
            required_tags: vec!["rust".to_string()],
            excluded_tags: vec![],
            min_resources: None,
            runner_type: None,
            node_affinity: vec![],
        },
        priority: JobPriority::Normal,
        timeout: Duration::from_secs(3600),
        retry_policy: RetryPolicy {
            max_retries: 3,
            retry_delay: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
        },
        metadata: std::collections::HashMap::new(),
        created_at: Utc::now(),
        scheduled_at: None,
    }
}

/// Create a test runner for performance testing
fn create_test_runner() -> RunnerEntity {
    RunnerEntity::new(
        "test-runner".to_string(),
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    )
}

#[tokio::test]
#[ignore] // Requires MongoDB
async fn test_job_repository_bulk_operations_performance() {
    let client = match Client::with_uri_str("mongodb://localhost:27017").await {
        Ok(client) => client,
        Err(_) => {
            println!("MongoDB not available for performance test");
            return;
        }
    };

    let database = client.database("test_performance_jobs");
    let config = ProductionDatabaseConfig::default();
    let db_manager = Arc::new(
        ProductionDatabaseManager::new(client.clone(), database.clone(), config)
            .await
            .expect("Failed to create database manager")
    );

    let repo = EnhancedMongoJobRepository::new(&database, db_manager)
        .await
        .expect("Failed to create repository");

    // Test bulk creation performance
    let job_count = 1000;
    let jobs: Vec<Job> = (0..job_count).map(|_| create_test_job()).collect();

    let start_time = Instant::now();
    let result = repo.bulk_create_optimized(&jobs).await;
    let duration = start_time.elapsed();

    assert!(result.is_ok());
    let job_ids = result.unwrap();
    assert_eq!(job_ids.len(), job_count);

    println!("Bulk created {} jobs in {:?} ({:.2} jobs/sec)", 
             job_count, duration, job_count as f64 / duration.as_secs_f64());

    // Test bulk query performance
    let start_time = Instant::now();
    let queued_jobs = repo.find_next_jobs_optimized(100).await;
    let query_duration = start_time.elapsed();

    assert!(queued_jobs.is_ok());
    println!("Queried next 100 jobs in {:?}", query_duration);

    // Test performance metrics
    let metrics = repo.get_performance_metrics().await;
    println!("Repository metrics: {:?}", metrics);
    assert!(metrics.total_operations > 0);
    assert!(metrics.successful_operations > 0);

    // Cleanup
    for job_id in job_ids {
        let _ = repo.delete(job_id).await;
    }
}

#[tokio::test]
#[ignore] // Requires MongoDB
async fn test_runner_repository_performance() {
    let client = match Client::with_uri_str("mongodb://localhost:27017").await {
        Ok(client) => client,
        Err(_) => {
            println!("MongoDB not available for performance test");
            return;
        }
    };

    let database = client.database("test_performance_runners");
    let config = ProductionDatabaseConfig::default();
    let db_manager = Arc::new(
        ProductionDatabaseManager::new(client.clone(), database.clone(), config)
            .await
            .expect("Failed to create database manager")
    );

    let repo = EnhancedMongoRunnerRepository::new(&database, db_manager)
        .await
        .expect("Failed to create repository");

    // Test runner creation performance
    let runner_count = 100;
    let mut runners = Vec::new();

    let start_time = Instant::now();
    for i in 0..runner_count {
        let mut runner = create_test_runner();
        runner.name = format!("test-runner-{}", i);
        runner.tags = vec!["rust".to_string(), "performance".to_string()];
        
        repo.store_runner(&runner).await.expect("Failed to store runner");
        runners.push(runner);
    }
    let creation_duration = start_time.elapsed();

    println!("Created {} runners in {:?} ({:.2} runners/sec)", 
             runner_count, creation_duration, runner_count as f64 / creation_duration.as_secs_f64());

    // Test bulk query performance
    let start_time = Instant::now();
    let filter = EnhancedRunnerFilter {
        tags: Some(vec!["rust".to_string()]),
        limit: Some(50),
        ..Default::default()
    };
    let found_runners = repo.list_runners(filter).await;
    let query_duration = start_time.elapsed();

    assert!(found_runners.is_ok());
    let found_runners = found_runners.unwrap();
    println!("Queried {} runners with tags in {:?}", found_runners.len(), query_duration);

    // Test heartbeat update performance
    let start_time = Instant::now();
    for runner in &runners {
        repo.update_runner_heartbeat(runner.id).await.expect("Failed to update heartbeat");
    }
    let heartbeat_duration = start_time.elapsed();

    println!("Updated {} runner heartbeats in {:?} ({:.2} updates/sec)", 
             runner_count, heartbeat_duration, runner_count as f64 / heartbeat_duration.as_secs_f64());

    // Test status update performance
    let start_time = Instant::now();
    for runner in &runners {
        repo.update_runner_status(runner.id, RunnerStatus::Busy).await.expect("Failed to update status");
    }
    let status_duration = start_time.elapsed();

    println!("Updated {} runner statuses in {:?} ({:.2} updates/sec)", 
             runner_count, status_duration, runner_count as f64 / status_duration.as_secs_f64());

    // Test statistics performance
    let start_time = Instant::now();
    let stats = repo.get_runner_statistics().await;
    let stats_duration = start_time.elapsed();

    assert!(stats.is_ok());
    let stats = stats.unwrap();
    println!("Generated runner statistics in {:?}: {:?}", stats_duration, stats);

    // Test performance metrics
    let metrics = repo.get_performance_metrics().await;
    println!("Runner repository metrics: {:?}", metrics);
    assert!(metrics.total_operations > 0);
    assert!(metrics.successful_operations > 0);
    assert!(metrics.heartbeat_updates > 0);
    assert!(metrics.status_updates > 0);

    // Cleanup
    for runner in runners {
        let _ = repo.remove_runner(runner.id).await;
    }
}

#[tokio::test]
#[ignore] // Requires MongoDB
async fn test_concurrent_operations_performance() {
    let client = match Client::with_uri_str("mongodb://localhost:27017").await {
        Ok(client) => client,
        Err(_) => {
            println!("MongoDB not available for performance test");
            return;
        }
    };

    let database = client.database("test_concurrent_performance");
    let config = ProductionDatabaseConfig::default();
    let db_manager = Arc::new(
        ProductionDatabaseManager::new(client.clone(), database.clone(), config)
            .await
            .expect("Failed to create database manager")
    );

    let job_repo = Arc::new(
        EnhancedMongoJobRepository::new(&database, db_manager.clone())
            .await
            .expect("Failed to create job repository")
    );

    let runner_repo = Arc::new(
        EnhancedMongoRunnerRepository::new(&database, db_manager)
            .await
            .expect("Failed to create runner repository")
    );

    // Test concurrent job operations
    let concurrent_tasks = 10;
    let jobs_per_task = 50;

    let start_time = Instant::now();
    let mut handles = Vec::new();

    for task_id in 0..concurrent_tasks {
        let repo = Arc::clone(&job_repo);
        let handle = tokio::spawn(async move {
            let jobs: Vec<Job> = (0..jobs_per_task)
                .map(|i| {
                    let mut job = create_test_job();
                    job.name = format!("concurrent-job-{}-{}", task_id, i);
                    job
                })
                .collect();

            repo.bulk_create_optimized(&jobs).await
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let mut total_jobs = 0;
    for handle in handles {
        let result = handle.await.expect("Task panicked");
        if let Ok(job_ids) = result {
            total_jobs += job_ids.len();
        }
    }

    let concurrent_duration = start_time.elapsed();
    println!("Concurrently created {} jobs in {:?} ({:.2} jobs/sec)", 
             total_jobs, concurrent_duration, total_jobs as f64 / concurrent_duration.as_secs_f64());

    // Test concurrent runner operations
    let start_time = Instant::now();
    let mut handles = Vec::new();

    for task_id in 0..concurrent_tasks {
        let repo = Arc::clone(&runner_repo);
        let handle = tokio::spawn(async move {
            for i in 0..10 {
                let mut runner = create_test_runner();
                runner.name = format!("concurrent-runner-{}-{}", task_id, i);
                
                if let Err(e) = repo.store_runner(&runner).await {
                    eprintln!("Failed to store runner: {}", e);
                    continue;
                }

                // Update heartbeat
                if let Err(e) = repo.update_runner_heartbeat(runner.id).await {
                    eprintln!("Failed to update heartbeat: {}", e);
                }

                // Update status
                if let Err(e) = repo.update_runner_status(runner.id, RunnerStatus::Idle).await {
                    eprintln!("Failed to update status: {}", e);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let runner_concurrent_duration = start_time.elapsed();
    println!("Concurrently processed runner operations in {:?}", runner_concurrent_duration);

    // Verify final metrics
    let job_metrics = job_repo.get_performance_metrics().await;
    let runner_metrics = runner_repo.get_performance_metrics().await;

    println!("Final job repository metrics: {:?}", job_metrics);
    println!("Final runner repository metrics: {:?}", runner_metrics);

    assert!(job_metrics.total_operations > 0);
    assert!(runner_metrics.total_operations > 0);
}

#[tokio::test]
async fn test_repository_error_handling() {
    // Test error handling without requiring MongoDB
    use rustci::error::AppError;

    // Test invalid UUID parsing
    let invalid_uuid = "invalid-uuid";
    let parse_result = Uuid::parse_str(invalid_uuid);
    assert!(parse_result.is_err());

    // Test error creation
    let db_error = AppError::DatabaseError("Test database error".to_string());
    match db_error {
        AppError::DatabaseError(msg) => {
            assert_eq!(msg, "Test database error");
        }
        _ => panic!("Expected DatabaseError"),
    }

    // Test not found error
    let not_found_error = AppError::NotFound("Resource not found".to_string());
    match not_found_error {
        AppError::NotFound(msg) => {
            assert_eq!(msg, "Resource not found");
        }
        _ => panic!("Expected NotFound error"),
    }
}

#[tokio::test]
async fn test_filter_building() {
    // Test enhanced filter building logic
    let filter = EnhancedRunnerFilter {
        status: Some(RunnerStatus::Idle),
        runner_type: Some("Local".to_string()),
        tags: Some(vec!["rust".to_string(), "test".to_string()]),
        capacity_min: Some(2),
        capacity_max: Some(8),
        last_heartbeat_after: Some(Utc::now() - chrono::Duration::minutes(5)),
        limit: Some(10),
        skip: Some(0),
        sort_by: Some("capacity".to_string()),
        sort_order: Some(-1),
    };

    // Verify filter properties
    assert_eq!(filter.status, Some(RunnerStatus::Idle));
    assert_eq!(filter.runner_type, Some("Local".to_string()));
    assert_eq!(filter.tags.as_ref().unwrap().len(), 2);
    assert_eq!(filter.capacity_min, Some(2));
    assert_eq!(filter.capacity_max, Some(8));
    assert_eq!(filter.limit, Some(10));
    assert_eq!(filter.skip, Some(0));
    assert_eq!(filter.sort_by, Some("capacity".to_string()));
    assert_eq!(filter.sort_order, Some(-1));
}