use crate::common::*;
use rand::Rng;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Chaos experiment configuration
#[derive(Debug, Clone)]
pub struct ChaosExperiment {
    pub name: String,
    pub description: String,
    pub duration: Duration,
    pub failure_rate: f64, // 0.0 to 1.0
    pub recovery_time: Duration,
    pub steady_state_hypothesis: Box<dyn SteadyStateHypothesis>,
    pub method: Box<dyn ChaosMethod>,
}

/// Trait for defining steady state hypothesis
#[async_trait::async_trait]
pub trait SteadyStateHypothesis: Send + Sync {
    async fn verify(&self, env: &TestEnvironment) -> TestResult<bool>;
    fn description(&self) -> &str;
}

/// Trait for chaos methods (what failures to inject)
#[async_trait::async_trait]
pub trait ChaosMethod: Send + Sync {
    async fn inject_failure(&self, env: &TestEnvironment) -> TestResult<()>;
    async fn recover(&self, env: &TestEnvironment) -> TestResult<()>;
    fn description(&self) -> &str;
}

/// Chaos experiment results
#[derive(Debug)]
pub struct ChaosResults {
    pub experiment_name: String,
    pub duration: Duration,
    pub steady_state_maintained: bool,
    pub failure_injection_successful: bool,
    pub recovery_successful: bool,
    pub observations: Vec<ChaosObservation>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ChaosObservation {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metric: String,
    pub value: f64,
    pub status: String,
}

/// Main chaos testing runner
pub struct ChaosRunner {
    env: TestEnvironment,
    experiments: Vec<ChaosExperiment>,
    observations: Arc<RwLock<Vec<ChaosObservation>>>,
}

impl ChaosRunner {
    pub async fn new() -> Self {
        Self {
            env: TestEnvironment::new().await,
            experiments: Vec::new(),
            observations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add_experiment(&mut self, experiment: ChaosExperiment) {
        self.experiments.push(experiment);
    }

    /// Run a single chaos experiment
    pub async fn run_experiment(&self, experiment: &ChaosExperiment) -> TestResult<ChaosResults> {
        println!("🔥 Starting chaos experiment: {}", experiment.name);
        println!("📝 Description: {}", experiment.description);

        let start_time = Instant::now();
        let mut results = ChaosResults {
            experiment_name: experiment.name.clone(),
            duration: Duration::ZERO,
            steady_state_maintained: false,
            failure_injection_successful: false,
            recovery_successful: false,
            observations: Vec::new(),
            errors: Vec::new(),
        };

        // Phase 1: Verify steady state before experiment
        println!("🔍 Phase 1: Verifying steady state hypothesis...");
        match experiment.steady_state_hypothesis.verify(&self.env).await {
            Ok(true) => {
                println!("✅ Steady state verified");
            }
            Ok(false) => {
                let error = "Steady state hypothesis failed before experiment";
                println!("❌ {}", error);
                results.errors.push(error.to_string());
                return Ok(results);
            }
            Err(e) => {
                let error = format!("Error verifying steady state: {}", e);
                println!("❌ {}", error);
                results.errors.push(error);
                return Ok(results);
            }
        }

        // Phase 2: Inject failure
        println!("💥 Phase 2: Injecting failure...");
        match experiment.method.inject_failure(&self.env).await {
            Ok(()) => {
                println!("✅ Failure injected successfully");
                results.failure_injection_successful = true;
            }
            Err(e) => {
                let error = format!("Failed to inject failure: {}", e);
                println!("❌ {}", error);
                results.errors.push(error);
            }
        }

        // Phase 3: Monitor system during chaos
        println!("📊 Phase 3: Monitoring system behavior...");
        let monitoring_handle = self.start_monitoring();

        // Let the chaos run for the specified duration
        tokio::time::sleep(experiment.duration).await;

        // Phase 4: Verify steady state during chaos
        println!("🔍 Phase 4: Verifying steady state during chaos...");
        match experiment.steady_state_hypothesis.verify(&self.env).await {
            Ok(maintained) => {
                results.steady_state_maintained = maintained;
                if maintained {
                    println!("✅ System maintained steady state during chaos");
                } else {
                    println!("⚠️ System did not maintain steady state during chaos");
                }
            }
            Err(e) => {
                let error = format!("Error verifying steady state during chaos: {}", e);
                println!("❌ {}", error);
                results.errors.push(error);
            }
        }

        // Phase 5: Recover from failure
        println!("🔧 Phase 5: Recovering from failure...");
        match experiment.method.recover(&self.env).await {
            Ok(()) => {
                println!("✅ Recovery successful");
                results.recovery_successful = true;
            }
            Err(e) => {
                let error = format!("Recovery failed: {}", e);
                println!("❌ {}", error);
                results.errors.push(error);
            }
        }

        // Wait for recovery time
        tokio::time::sleep(experiment.recovery_time).await;

        // Phase 6: Verify steady state after recovery
        println!("🔍 Phase 6: Verifying steady state after recovery...");
        match experiment.steady_state_hypothesis.verify(&self.env).await {
            Ok(true) => {
                println!("✅ System returned to steady state after recovery");
            }
            Ok(false) => {
                let error = "System did not return to steady state after recovery";
                println!("❌ {}", error);
                results.errors.push(error.to_string());
            }
            Err(e) => {
                let error = format!("Error verifying steady state after recovery: {}", e);
                println!("❌ {}", error);
                results.errors.push(error);
            }
        }

        // Stop monitoring and collect observations
        monitoring_handle.abort();
        results.observations = self.observations.read().await.clone();
        results.duration = start_time.elapsed();

        self.print_experiment_results(&results);
        Ok(results)
    }

    /// Run all configured experiments
    pub async fn run_all_experiments(&self) -> TestResult<Vec<ChaosResults>> {
        let mut all_results = Vec::new();

        for experiment in &self.experiments {
            let results = self.run_experiment(experiment).await?;
            all_results.push(results);

            // Wait between experiments
            tokio::time::sleep(Duration::from_secs(30)).await;
        }

        self.print_summary(&all_results);
        Ok(all_results)
    }

    fn start_monitoring(&self) -> tokio::task::JoinHandle<()> {
        let observations = self.observations.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                // Collect various metrics
                let timestamp = chrono::Utc::now();
                let mut obs = observations.write().await;

                // Simulate metric collection
                obs.push(ChaosObservation {
                    timestamp,
                    metric: "response_time_ms".to_string(),
                    value: rand::thread_rng().gen_range(50.0..200.0),
                    status: "ok".to_string(),
                });

                obs.push(ChaosObservation {
                    timestamp,
                    metric: "error_rate".to_string(),
                    value: rand::thread_rng().gen_range(0.0..0.1),
                    status: "ok".to_string(),
                });

                obs.push(ChaosObservation {
                    timestamp,
                    metric: "throughput_rps".to_string(),
                    value: rand::thread_rng().gen_range(80.0..120.0),
                    status: "ok".to_string(),
                });
            }
        })
    }

    fn print_experiment_results(&self, results: &ChaosResults) {
        println!("\n🔥 Chaos Experiment Results: {}", results.experiment_name);
        println!("⏱️ Duration: {:?}", results.duration);
        println!(
            "🎯 Steady State Maintained: {}",
            results.steady_state_maintained
        );
        println!(
            "💥 Failure Injection Successful: {}",
            results.failure_injection_successful
        );
        println!("🔧 Recovery Successful: {}", results.recovery_successful);

        if !results.errors.is_empty() {
            println!("❌ Errors:");
            for error in &results.errors {
                println!("   - {}", error);
            }
        }

        println!(
            "📊 Observations: {} data points collected",
            results.observations.len()
        );
        println!("{'=':<50}");
    }

    fn print_summary(&self, all_results: &[ChaosResults]) {
        println!("\n🔥 CHAOS TESTING SUMMARY");
        println!("{'=':<50}");

        let total_experiments = all_results.len();
        let successful_experiments = all_results
            .iter()
            .filter(|r| r.steady_state_maintained && r.recovery_successful)
            .count();

        println!("📊 Total Experiments: {}", total_experiments);
        println!("✅ Successful Experiments: {}", successful_experiments);
        println!(
            "❌ Failed Experiments: {}",
            total_experiments - successful_experiments
        );
        println!(
            "📈 Success Rate: {:.1}%",
            (successful_experiments as f64 / total_experiments as f64) * 100.0
        );

        println!("\n📋 Experiment Details:");
        for result in all_results {
            let status = if result.steady_state_maintained && result.recovery_successful {
                "✅ PASS"
            } else {
                "❌ FAIL"
            };
            println!("   {} - {}", status, result.experiment_name);
        }

        println!("{'=':<50}");
    }
}

// Example steady state hypothesis implementations
pub struct ResponseTimeHypothesis {
    max_response_time_ms: u64,
}

impl ResponseTimeHypothesis {
    pub fn new(max_response_time_ms: u64) -> Self {
        Self {
            max_response_time_ms,
        }
    }
}

#[async_trait::async_trait]
impl SteadyStateHypothesis for ResponseTimeHypothesis {
    async fn verify(&self, _env: &TestEnvironment) -> TestResult<bool> {
        // Simulate checking response time
        let current_response_time = rand::thread_rng().gen_range(50..300);
        Ok(current_response_time < self.max_response_time_ms)
    }

    fn description(&self) -> &str {
        "Response time should be under threshold"
    }
}

pub struct AvailabilityHypothesis {
    min_success_rate: f64,
}

impl AvailabilityHypothesis {
    pub fn new(min_success_rate: f64) -> Self {
        Self { min_success_rate }
    }
}

#[async_trait::async_trait]
impl SteadyStateHypothesis for AvailabilityHypothesis {
    async fn verify(&self, _env: &TestEnvironment) -> TestResult<bool> {
        // Simulate checking success rate
        let current_success_rate = rand::thread_rng().gen_range(0.8..1.0);
        Ok(current_success_rate >= self.min_success_rate)
    }

    fn description(&self) -> &str {
        "System availability should be above threshold"
    }
}

// Example chaos method implementations
pub struct DatabaseFailureMethod {
    failure_duration: Duration,
}

impl DatabaseFailureMethod {
    pub fn new(failure_duration: Duration) -> Self {
        Self { failure_duration }
    }
}

#[async_trait::async_trait]
impl ChaosMethod for DatabaseFailureMethod {
    async fn inject_failure(&self, env: &TestEnvironment) -> TestResult<()> {
        println!("💾 Injecting database failure...");
        env.database.set_should_fail(true).await;
        Ok(())
    }

    async fn recover(&self, env: &TestEnvironment) -> TestResult<()> {
        println!("💾 Recovering database...");
        env.database.set_should_fail(false).await;
        Ok(())
    }

    fn description(&self) -> &str {
        "Inject database connection failures"
    }
}

pub struct NetworkLatencyMethod {
    latency_ms: u64,
}

impl NetworkLatencyMethod {
    pub fn new(latency_ms: u64) -> Self {
        Self { latency_ms }
    }
}

#[async_trait::async_trait]
impl ChaosMethod for NetworkLatencyMethod {
    async fn inject_failure(&self, _env: &TestEnvironment) -> TestResult<()> {
        println!("🌐 Injecting network latency: {}ms", self.latency_ms);
        // In a real implementation, you would configure network delays
        Ok(())
    }

    async fn recover(&self, _env: &TestEnvironment) -> TestResult<()> {
        println!("🌐 Removing network latency");
        // In a real implementation, you would remove network delays
        Ok(())
    }

    fn description(&self) -> &str {
        "Inject network latency"
    }
}
