use crate::common::*;
use futures::future::join_all;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Load test configuration
#[derive(Debug, Clone)]
pub struct LoadTestConfig {
    pub concurrent_users: usize,
    pub requests_per_user: usize,
    pub ramp_up_duration: Duration,
    pub test_duration: Duration,
    pub think_time: Duration,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            concurrent_users: 10,
            requests_per_user: 100,
            ramp_up_duration: Duration::from_secs(30),
            test_duration: Duration::from_secs(300), // 5 minutes
            think_time: Duration::from_millis(100),
        }
    }
}

/// Load test results
#[derive(Debug)]
pub struct LoadTestResults {
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub average_response_time: Duration,
    pub min_response_time: Duration,
    pub max_response_time: Duration,
    pub p95_response_time: Duration,
    pub requests_per_second: f64,
    pub errors: Vec<String>,
}

/// Pipeline execution load test
pub struct PipelineLoadTest {
    config: LoadTestConfig,
    env: TestEnvironment,
}

impl PipelineLoadTest {
    pub async fn new(config: LoadTestConfig) -> Self {
        Self {
            config,
            env: TestEnvironment::new().await,
        }
    }

    /// Run load test for pipeline creation
    pub async fn test_pipeline_creation(&self) -> TestResult<LoadTestResults> {
        println!("Starting pipeline creation load test...");
        println!("Config: {:?}", self.config);

        let start_time = Instant::now();
        let success_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));
        let response_times = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let errors = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Create test users
        let mut users = Vec::new();
        for i in 0..self.config.concurrent_users {
            let user = TestDataBuilder::user()
                .with_username(&format!("loadtest_user_{}", i))
                .with_email(&format!("loadtest_user_{}@example.com", i))
                .build();

            self.env
                .database
                .insert("users", TestDocument::new(serde_json::to_value(&user)?))
                .await?;
            users.push(user);
        }

        // Spawn concurrent user tasks
        let mut handles = Vec::new();
        for (user_index, user) in users.into_iter().enumerate() {
            let success_count = success_count.clone();
            let error_count = error_count.clone();
            let response_times = response_times.clone();
            let errors = errors.clone();
            let config = self.config.clone();

            let handle = tokio::spawn(async move {
                // Stagger user start times for ramp-up
                let delay = config
                    .ramp_up_duration
                    .mul_f64(user_index as f64 / config.concurrent_users as f64);
                sleep(delay).await;

                for request_index in 0..config.requests_per_user {
                    let request_start = Instant::now();

                    // Create pipeline
                    let pipeline_data = TestDataBuilder::pipeline()
                        .with_name(&format!(
                            "load_test_pipeline_{}_{}",
                            user_index, request_index
                        ))
                        .build();

                    match Self::create_pipeline_request(&pipeline_data).await {
                        Ok(_) => {
                            success_count.fetch_add(1, Ordering::Relaxed);
                            let response_time = request_start.elapsed();
                            response_times.lock().await.push(response_time);
                        }
                        Err(e) => {
                            error_count.fetch_add(1, Ordering::Relaxed);
                            errors.lock().await.push(format!(
                                "User {}, Request {}: {}",
                                user_index, request_index, e
                            ));
                        }
                    }

                    // Think time between requests
                    sleep(config.think_time).await;

                    // Check if test duration exceeded
                    if start_time.elapsed() > config.test_duration {
                        break;
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete or timeout
        let timeout_duration = self.config.test_duration + Duration::from_secs(30);
        match tokio::time::timeout(timeout_duration, join_all(handles)).await {
            Ok(_) => println!("All load test tasks completed"),
            Err(_) => println!("Load test timed out"),
        }

        // Calculate results
        let total_duration = start_time.elapsed();
        let successful_requests = success_count.load(Ordering::Relaxed);
        let failed_requests = error_count.load(Ordering::Relaxed);
        let total_requests = successful_requests + failed_requests;

        let response_times_vec = response_times.lock().await.clone();
        let errors_vec = errors.lock().await.clone();

        let results = Self::calculate_results(
            total_requests,
            successful_requests,
            failed_requests,
            response_times_vec,
            total_duration,
            errors_vec,
        );

        self.print_results(&results);
        Ok(results)
    }

    /// Run load test for pipeline execution
    pub async fn test_pipeline_execution(&self) -> TestResult<LoadTestResults> {
        println!("Starting pipeline execution load test...");

        // Setup test data
        let user = TestDataBuilder::user().build();
        let workspace = TestDataBuilder::workspace().with_owner(user.id).build();
        let pipeline = TestDataBuilder::pipeline().build();

        self.env
            .database
            .insert("users", TestDocument::new(serde_json::to_value(&user)?))
            .await?;
        self.env
            .database
            .insert(
                "workspaces",
                TestDocument::new(serde_json::to_value(&workspace)?),
            )
            .await?;
        self.env
            .database
            .insert(
                "pipelines",
                TestDocument::new(serde_json::to_value(&pipeline)?),
            )
            .await?;

        let start_time = Instant::now();
        let success_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));
        let response_times = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let errors = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Spawn concurrent execution tasks
        let mut handles = Vec::new();
        for user_index in 0..self.config.concurrent_users {
            let pipeline_id = pipeline.id;
            let success_count = success_count.clone();
            let error_count = error_count.clone();
            let response_times = response_times.clone();
            let errors = errors.clone();
            let config = self.config.clone();

            let handle = tokio::spawn(async move {
                let delay = config
                    .ramp_up_duration
                    .mul_f64(user_index as f64 / config.concurrent_users as f64);
                sleep(delay).await;

                for request_index in 0..config.requests_per_user {
                    let request_start = Instant::now();

                    let execution_context = TestDataBuilder::execution_context()
                        .with_pipeline_id(pipeline_id)
                        .build();

                    match Self::execute_pipeline_request(&execution_context).await {
                        Ok(_) => {
                            success_count.fetch_add(1, Ordering::Relaxed);
                            let response_time = request_start.elapsed();
                            response_times.lock().await.push(response_time);
                        }
                        Err(e) => {
                            error_count.fetch_add(1, Ordering::Relaxed);
                            errors.lock().await.push(format!(
                                "User {}, Request {}: {}",
                                user_index, request_index, e
                            ));
                        }
                    }

                    sleep(config.think_time).await;

                    if start_time.elapsed() > config.test_duration {
                        break;
                    }
                }
            });

            handles.push(handle);
        }

        let timeout_duration = self.config.test_duration + Duration::from_secs(30);
        let _ = tokio::time::timeout(timeout_duration, join_all(handles)).await;

        let total_duration = start_time.elapsed();
        let successful_requests = success_count.load(Ordering::Relaxed);
        let failed_requests = error_count.load(Ordering::Relaxed);
        let total_requests = successful_requests + failed_requests;

        let response_times_vec = response_times.lock().await.clone();
        let errors_vec = errors.lock().await.clone();

        let results = Self::calculate_results(
            total_requests,
            successful_requests,
            failed_requests,
            response_times_vec,
            total_duration,
            errors_vec,
        );

        self.print_results(&results);
        Ok(results)
    }

    async fn create_pipeline_request(pipeline: &TestPipeline) -> TestResult<()> {
        // Simulate pipeline creation API call
        sleep(Duration::from_millis(50)).await; // Simulate processing time

        // Simulate occasional failures (5% failure rate)
        if rand::random::<f64>() < 0.05 {
            return Err("Simulated pipeline creation failure".into());
        }

        Ok(())
    }

    async fn execute_pipeline_request(context: &TestExecutionContext) -> TestResult<()> {
        // Simulate pipeline execution API call
        sleep(Duration::from_millis(100)).await; // Simulate processing time

        // Simulate occasional failures (3% failure rate)
        if rand::random::<f64>() < 0.03 {
            return Err("Simulated pipeline execution failure".into());
        }

        Ok(())
    }

    fn calculate_results(
        total_requests: usize,
        successful_requests: usize,
        failed_requests: usize,
        mut response_times: Vec<Duration>,
        total_duration: Duration,
        errors: Vec<String>,
    ) -> LoadTestResults {
        response_times.sort();

        let average_response_time = if !response_times.is_empty() {
            response_times.iter().sum::<Duration>() / response_times.len() as u32
        } else {
            Duration::ZERO
        };

        let min_response_time = response_times.first().copied().unwrap_or(Duration::ZERO);
        let max_response_time = response_times.last().copied().unwrap_or(Duration::ZERO);

        let p95_index = (response_times.len() as f64 * 0.95) as usize;
        let p95_response_time = response_times
            .get(p95_index)
            .copied()
            .unwrap_or(Duration::ZERO);

        let requests_per_second = if total_duration.as_secs() > 0 {
            total_requests as f64 / total_duration.as_secs_f64()
        } else {
            0.0
        };

        LoadTestResults {
            total_requests,
            successful_requests,
            failed_requests,
            average_response_time,
            min_response_time,
            max_response_time,
            p95_response_time,
            requests_per_second,
            errors,
        }
    }

    fn print_results(&self, results: &LoadTestResults) {
        println!("\n=== Load Test Results ===");
        println!("Total Requests: {}", results.total_requests);
        println!("Successful Requests: {}", results.successful_requests);
        println!("Failed Requests: {}", results.failed_requests);
        println!(
            "Success Rate: {:.2}%",
            (results.successful_requests as f64 / results.total_requests as f64) * 100.0
        );
        println!("Average Response Time: {:?}", results.average_response_time);
        println!("Min Response Time: {:?}", results.min_response_time);
        println!("Max Response Time: {:?}", results.max_response_time);
        println!("P95 Response Time: {:?}", results.p95_response_time);
        println!("Requests per Second: {:.2}", results.requests_per_second);

        if !results.errors.is_empty() {
            println!("\nErrors (first 10):");
            for (i, error) in results.errors.iter().take(10).enumerate() {
                println!("  {}: {}", i + 1, error);
            }
            if results.errors.len() > 10 {
                println!("  ... and {} more errors", results.errors.len() - 10);
            }
        }
        println!("========================\n");
    }
}

#[tokio::test]
async fn test_pipeline_creation_load() -> TestResult {
    let config = LoadTestConfig {
        concurrent_users: 5,
        requests_per_user: 20,
        ramp_up_duration: Duration::from_secs(10),
        test_duration: Duration::from_secs(60),
        think_time: Duration::from_millis(50),
    };

    let load_test = PipelineLoadTest::new(config).await;
    let results = load_test.test_pipeline_creation().await?;

    // Assert performance thresholds
    assert!(results.average_response_time < Duration::from_millis(200));
    assert!(results.p95_response_time < Duration::from_millis(500));
    assert!(results.requests_per_second > 10.0);
    assert!((results.successful_requests as f64 / results.total_requests as f64) > 0.95);

    Ok(())
}

#[tokio::test]
async fn test_pipeline_execution_load() -> TestResult {
    let config = LoadTestConfig {
        concurrent_users: 3,
        requests_per_user: 10,
        ramp_up_duration: Duration::from_secs(5),
        test_duration: Duration::from_secs(30),
        think_time: Duration::from_millis(100),
    };

    let load_test = PipelineLoadTest::new(config).await;
    let results = load_test.test_pipeline_execution().await?;

    // Assert performance thresholds for execution
    assert!(results.average_response_time < Duration::from_millis(300));
    assert!(results.p95_response_time < Duration::from_millis(800));
    assert!((results.successful_requests as f64 / results.total_requests as f64) > 0.97);

    Ok(())
}
