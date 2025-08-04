use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use RustAutoDevOps::core::{
    control_plane_observability::{ControlPlaneObservability, ObservabilityConfig},
    distributed_tracing::JobTraceStatus,
};

#[tokio::test]
async fn test_comprehensive_observability_integration() {
    // Initialize observability with all features enabled
    let config = ObservabilityConfig {
        service_name: "test-control-plane".to_string(),
        environment: "test".to_string(),
        enable_health_checks: true,
        enable_metrics: true,
        enable_tracing: true,
        enable_structured_logging: true,
        ..Default::default()
    };

    let observability = Arc::new(
        ControlPlaneObservability::new(config)
            .await
            .expect("Failed to create observability service")
    );

    // Test initial status
    let initial_status = observability.get_observability_status().await;
    assert_eq!(initial_status.service_name, "test-control-plane");
    assert_eq!(initial_status.environment, "test");
    assert_eq!(initial_status.active_job_traces, 0);

    // Test job observability lifecycle
    let job_id = Uuid::new_v4();
    let mut job_context = observability
        .start_job_observability(
            job_id,
            Some("test-pipeline".to_string()),
            Some("test-node-1".to_string()),
        )
        .await
        .expect("Failed to start job observability");

    // Verify job trace was created
    let status_after_start = observability.get_observability_status().await;
    assert_eq!(status_after_start.active_job_traces, 1);
    assert!(status_after_start.metrics.jobs_scheduled_total >= 1);

    // Update job status to running
    observability
        .update_job_status(
            &mut job_context,
            JobTraceStatus::Running,
            Some("test-node-1".to_string()),
            None,
        )
        .await
        .expect("Failed to update job status");

    // Simulate some work
    sleep(Duration::from_millis(100)).await;

    // Complete job successfully
    observability
        .complete_job_observability(job_context, true, Some("test-node-1".to_string()))
        .await
        .expect("Failed to complete job observability");

    // Verify job completion
    let final_status = observability.get_observability_status().await;
    assert_eq!(final_status.active_job_traces, 0);
    assert!(final_status.metrics.jobs_completed_total >= 1);
}

#[tokio::test]
async fn test_node_event_observability() {
    let config = ObservabilityConfig::default();
    let observability = ControlPlaneObservability::new(config)
        .await
        .expect("Failed to create observability service");

    // Test node join event
    let mut resource_usage = HashMap::new();
    resource_usage.insert("cpu_percent".to_string(), 45.0);
    resource_usage.insert("memory_percent".to_string(), 60.0);
    resource_usage.insert("disk_percent".to_string(), 30.0);

    observability
        .record_node_event(
            "test-node-1".to_string(),
            "node_joined",
            "Node successfully joined the cluster",
            Some("healthy".to_string()),
            Some(resource_usage.clone()),
        )
        .await
        .expect("Failed to record node join event");

    // Test node health change event
    observability
        .record_node_event(
            "test-node-1".to_string(),
            "node_health_changed",
            "Node health status changed to degraded",
            Some("degraded".to_string()),
            Some(resource_usage),
        )
        .await
        .expect("Failed to record node health change event");

    // Test node leave event
    observability
        .record_node_event(
            "test-node-1".to_string(),
            "node_left",
            "Node left the cluster due to maintenance",
            Some("offline".to_string()),
            None,
        )
        .await
        .expect("Failed to record node leave event");

    // Verify events were recorded in logs
    let recent_logs = observability.structured_logging().get_recent_logs(10).await;
    let node_events: Vec<_> = recent_logs
        .iter()
        .filter(|log| {
            log.attributes
                .get("node_id")
                .and_then(|v| v.as_str())
                .map(|s| s == "test-node-1")
                .unwrap_or(false)
        })
        .collect();

    assert!(node_events.len() >= 3, "Expected at least 3 node events in logs");
}

#[tokio::test]
async fn test_failed_job_observability() {
    let config = ObservabilityConfig::default();
    let observability = ControlPlaneObservability::new(config)
        .await
        .expect("Failed to create observability service");

    // Start job observability
    let job_id = Uuid::new_v4();
    let mut job_context = observability
        .start_job_observability(
            job_id,
            Some("failing-pipeline".to_string()),
            Some("test-node-1".to_string()),
        )
        .await
        .expect("Failed to start job observability");

    // Update to running
    observability
        .update_job_status(
            &mut job_context,
            JobTraceStatus::Running,
            Some("test-node-1".to_string()),
            None,
        )
        .await
        .expect("Failed to update job status to running");

    // Simulate failure
    observability
        .update_job_status(
            &mut job_context,
            JobTraceStatus::Failed,
            Some("test-node-1".to_string()),
            Some("Build step failed with exit code 1".to_string()),
        )
        .await
        .expect("Failed to update job status to failed");

    // Complete job with failure
    observability
        .complete_job_observability(job_context, false, Some("test-node-1".to_string()))
        .await
        .expect("Failed to complete failed job observability");

    // Verify failure metrics
    let status = observability.get_observability_status().await;
    assert!(status.metrics.jobs_failed_total >= 1);
    assert_eq!(status.active_job_traces, 0);

    // Verify error logs
    let recent_logs = observability.structured_logging().get_recent_logs(20).await;
    let error_logs: Vec<_> = recent_logs
        .iter()
        .filter(|log| log.level == "error" && log.error.is_some())
        .collect();

    assert!(!error_logs.is_empty(), "Expected error logs for failed job");
}

#[tokio::test]
async fn test_multiple_concurrent_jobs() {
    let config = ObservabilityConfig::default();
    let observability = Arc::new(
        ControlPlaneObservability::new(config)
            .await
            .expect("Failed to create observability service")
    );

    let mut job_contexts = Vec::new();
    let job_count = 5;

    // Start multiple jobs concurrently
    for i in 0..job_count {
        let job_id = Uuid::new_v4();
        let job_context = observability
            .start_job_observability(
                job_id,
                Some(format!("pipeline-{}", i)),
                Some(format!("node-{}", i % 2 + 1)),
            )
            .await
            .expect("Failed to start job observability");

        job_contexts.push(job_context);
    }

    // Verify all jobs are tracked
    let status_with_active_jobs = observability.get_observability_status().await;
    assert_eq!(status_with_active_jobs.active_job_traces, job_count);

    // Complete all jobs
    for (i, mut context) in job_contexts.into_iter().enumerate() {
        let success = i % 3 != 0; // Make some jobs fail
        let node_id = format!("node-{}", i % 2 + 1);

        if success {
            observability
                .update_job_status(
                    &mut context,
                    JobTraceStatus::Running,
                    Some(node_id.clone()),
                    None,
                )
                .await
                .expect("Failed to update job status");

            observability
                .complete_job_observability(context, true, Some(node_id))
                .await
                .expect("Failed to complete successful job");
        } else {
            observability
                .update_job_status(
                    &mut context,
                    JobTraceStatus::Failed,
                    Some(node_id.clone()),
                    Some("Simulated failure".to_string()),
                )
                .await
                .expect("Failed to update job status to failed");

            observability
                .complete_job_observability(context, false, Some(node_id))
                .await
                .expect("Failed to complete failed job");
        }
    }

    // Verify final state
    let final_status = observability.get_observability_status().await;
    assert_eq!(final_status.active_job_traces, 0);
    assert!(final_status.metrics.jobs_completed_total >= 3); // At least 3 successful
    assert!(final_status.metrics.jobs_failed_total >= 2); // At least 2 failed
}

#[tokio::test]
async fn test_correlation_tracking() {
    let config = ObservabilityConfig::default();
    let observability = ControlPlaneObservability::new(config)
        .await
        .expect("Failed to create observability service");

    // Start job with correlation tracking
    let job_id = Uuid::new_v4();
    let job_context = observability
        .start_job_observability(
            job_id,
            Some("correlated-pipeline".to_string()),
            Some("test-node-1".to_string()),
        )
        .await
        .expect("Failed to start job observability");

    let correlation_id = job_context.correlation_id;

    // Record some node events with the same correlation
    observability
        .record_node_event(
            "test-node-1".to_string(),
            "resource_update",
            "Node resource usage updated",
            Some("healthy".to_string()),
            None,
        )
        .await
        .expect("Failed to record node event");

    // Complete the job
    observability
        .complete_job_observability(job_context, true, Some("test-node-1".to_string()))
        .await
        .expect("Failed to complete job");

    // Search logs by correlation ID
    let correlated_logs = observability
        .structured_logging()
        .search_logs_by_correlation(correlation_id)
        .await;

    assert!(!correlated_logs.is_empty(), "Expected correlated log entries");

    // Verify correlation ID is present in logs
    for log in &correlated_logs {
        assert_eq!(
            log.correlation_id.as_ref().unwrap(),
            &correlation_id.to_string()
        );
    }
}

#[tokio::test]
async fn test_metrics_collection_over_time() {
    let config = ObservabilityConfig {
        monitoring_interval_seconds: 1, // Fast interval for testing
        ..Default::default()
    };

    let observability = ControlPlaneObservability::new(config)
        .await
        .expect("Failed to create observability service");

    // Get initial metrics
    let initial_metrics = observability.metrics_collector().get_metrics_snapshot().await;

    // Simulate some activity
    for i in 0..3 {
        let job_id = Uuid::new_v4();
        let mut job_context = observability
            .start_job_observability(
                job_id,
                Some(format!("test-pipeline-{}", i)),
                Some("test-node-1".to_string()),
            )
            .await
            .expect("Failed to start job observability");

        observability
            .update_job_status(
                &mut job_context,
                JobTraceStatus::Running,
                Some("test-node-1".to_string()),
                None,
            )
            .await
            .expect("Failed to update job status");

        sleep(Duration::from_millis(50)).await;

        observability
            .complete_job_observability(job_context, true, Some("test-node-1".to_string()))
            .await
            .expect("Failed to complete job");
    }

    // Wait for metrics to be collected
    sleep(Duration::from_millis(1100)).await;

    // Get final metrics
    let final_metrics = observability.metrics_collector().get_metrics_snapshot().await;

    // Verify metrics increased
    assert!(final_metrics.jobs_scheduled_total >= initial_metrics.jobs_scheduled_total + 3);
    assert!(final_metrics.jobs_completed_total >= initial_metrics.jobs_completed_total + 3);
    assert!(final_metrics.uptime_seconds > initial_metrics.uptime_seconds);
}

#[tokio::test]
async fn test_health_check_integration() {
    let config = ObservabilityConfig::default();
    let observability = ControlPlaneObservability::new(config)
        .await
        .expect("Failed to create observability service");

    // Perform health check
    let health_response = observability.health_monitor().check_health().await;

    // Verify basic health response structure
    assert!(!health_response.version.is_empty());
    assert!(health_response.uptime_seconds > 0);
    assert!(!health_response.node_id.is_empty());

    // Verify system info is populated
    assert!(health_response.system_info.cpu_count > 0);
    assert!(health_response.system_info.memory_total_bytes > 0);

    // Health status should be healthy for a new system
    assert!(matches!(
        health_response.status,
        RustAutoDevOps::core::control_plane_health::HealthStatus::Healthy
    ));
}

#[tokio::test]
async fn test_logging_statistics() {
    let config = ObservabilityConfig::default();
    let observability = ControlPlaneObservability::new(config)
        .await
        .expect("Failed to create observability service");

    // Generate some log entries
    for i in 0..5 {
        let job_id = Uuid::new_v4();
        let job_context = observability
            .start_job_observability(
                job_id,
                Some(format!("stats-pipeline-{}", i)),
                Some("test-node-1".to_string()),
            )
            .await
            .expect("Failed to start job observability");

        observability
            .complete_job_observability(job_context, i % 2 == 0, Some("test-node-1".to_string()))
            .await
            .expect("Failed to complete job");
    }

    // Get logging statistics
    let stats = observability.structured_logging().get_statistics().await;

    // Verify statistics
    assert!(stats.total_entries > 0);
    assert!(stats.level_counts.contains_key("info"));
    assert!(stats.component_counts.contains_key("job_scheduler"));

    // Should have some errors from failed jobs
    if stats.error_count > 0 {
        assert!(stats.level_counts.contains_key("error"));
    }
}