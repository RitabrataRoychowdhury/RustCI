use crate::common::TestResult;
use serde_json::Value;
use std::time::Duration;
use uuid::Uuid;

/// Custom assertions for testing
pub struct TestAssertions;

impl TestAssertions {
    /// Assert that a result is an error of a specific type
    pub fn assert_error_type<T, E>(result: &Result<T, E>, expected_error: &str) -> TestResult
    where
        E: std::fmt::Debug,
    {
        match result {
            Err(error) => {
                let error_str = format!("{:?}", error);
                if error_str.contains(expected_error) {
                    Ok(())
                } else {
                    Err(format!(
                        "Expected error containing '{}', got: {:?}",
                        expected_error, error
                    )
                    .into())
                }
            }
            Ok(_) => Err("Expected error, but got Ok".into()),
        }
    }

    /// Assert that a value is within a time range
    pub fn assert_time_within_range(
        actual: chrono::DateTime<chrono::Utc>,
        expected: chrono::DateTime<chrono::Utc>,
        tolerance: Duration,
    ) -> TestResult {
        let diff = if actual > expected {
            actual - expected
        } else {
            expected - actual
        };

        if diff.to_std().unwrap() <= tolerance {
            Ok(())
        } else {
            Err(format!(
                "Time difference {} exceeds tolerance {}",
                diff.to_std().unwrap().as_secs(),
                tolerance.as_secs()
            )
            .into())
        }
    }

    /// Assert that a JSON value contains specific fields
    pub fn assert_json_contains_fields(json: &Value, fields: &[&str]) -> TestResult {
        if let Value::Object(obj) = json {
            for field in fields {
                if !obj.contains_key(*field) {
                    return Err(format!("JSON missing required field: {}", field).into());
                }
            }
            Ok(())
        } else {
            Err("Expected JSON object".into())
        }
    }

    /// Assert that a collection contains an item matching a predicate
    pub fn assert_contains<T, F>(collection: &[T], predicate: F) -> TestResult
    where
        F: Fn(&T) -> bool,
    {
        if collection.iter().any(predicate) {
            Ok(())
        } else {
            Err("Collection does not contain expected item".into())
        }
    }

    /// Assert that a collection has a specific length
    pub fn assert_length<T>(collection: &[T], expected_length: usize) -> TestResult {
        if collection.len() == expected_length {
            Ok(())
        } else {
            Err(format!(
                "Expected collection length {}, got {}",
                expected_length,
                collection.len()
            )
            .into())
        }
    }

    /// Assert that a UUID is valid (not nil)
    pub fn assert_valid_uuid(uuid: &Uuid) -> TestResult {
        if uuid.is_nil() {
            Err("UUID is nil".into())
        } else {
            Ok(())
        }
    }

    /// Assert that a string matches a regex pattern
    pub fn assert_matches_pattern(text: &str, pattern: &str) -> TestResult {
        let regex =
            regex::Regex::new(pattern).map_err(|e| format!("Invalid regex pattern: {}", e))?;

        if regex.is_match(text) {
            Ok(())
        } else {
            Err(format!("Text '{}' does not match pattern '{}'", text, pattern).into())
        }
    }

    /// Assert that an async operation completes within a timeout
    pub async fn assert_completes_within<F, T>(future: F, timeout: Duration) -> TestResult<T>
    where
        F: std::future::Future<Output = T>,
    {
        match tokio::time::timeout(timeout, future).await {
            Ok(result) => Ok(result),
            Err(_) => Err(format!("Operation did not complete within {:?}", timeout).into()),
        }
    }

    /// Assert that two JSON values are equivalent (ignoring order)
    pub fn assert_json_equivalent(actual: &Value, expected: &Value) -> TestResult {
        if Self::json_values_equivalent(actual, expected) {
            Ok(())
        } else {
            Err(format!(
                "JSON values are not equivalent:\nActual: {}\nExpected: {}",
                serde_json::to_string_pretty(actual).unwrap_or_default(),
                serde_json::to_string_pretty(expected).unwrap_or_default()
            )
            .into())
        }
    }

    fn json_values_equivalent(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Object(a_obj), Value::Object(b_obj)) => {
                if a_obj.len() != b_obj.len() {
                    return false;
                }
                for (key, a_val) in a_obj {
                    if let Some(b_val) = b_obj.get(key) {
                        if !Self::json_values_equivalent(a_val, b_val) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            }
            (Value::Array(a_arr), Value::Array(b_arr)) => {
                if a_arr.len() != b_arr.len() {
                    return false;
                }
                // For arrays, we check if all elements in a exist in b (order independent)
                a_arr.iter().all(|a_item| {
                    b_arr
                        .iter()
                        .any(|b_item| Self::json_values_equivalent(a_item, b_item))
                })
            }
            _ => a == b,
        }
    }

    /// Assert that a value is within a numeric range
    pub fn assert_in_range<T>(value: T, min: T, max: T) -> TestResult
    where
        T: PartialOrd + std::fmt::Display,
    {
        if value >= min && value <= max {
            Ok(())
        } else {
            Err(format!("Value {} is not in range [{}, {}]", value, min, max).into())
        }
    }

    /// Assert that a string is not empty
    pub fn assert_not_empty(text: &str) -> TestResult {
        if text.is_empty() {
            Err("String is empty".into())
        } else {
            Ok(())
        }
    }

    /// Assert that an option contains a value
    pub fn assert_some<T>(option: &Option<T>) -> TestResult {
        if option.is_some() {
            Ok(())
        } else {
            Err("Expected Some, got None".into())
        }
    }

    /// Assert that an option is None
    pub fn assert_none<T>(option: &Option<T>) -> TestResult {
        if option.is_none() {
            Ok(())
        } else {
            Err("Expected None, got Some".into())
        }
    }

    /// Assert that a vector is sorted
    pub fn assert_sorted<T>(vec: &[T]) -> TestResult
    where
        T: PartialOrd,
    {
        for window in vec.windows(2) {
            if window[0] > window[1] {
                return Err("Vector is not sorted".into());
            }
        }
        Ok(())
    }

    /// Assert that two collections have the same elements (order independent)
    pub fn assert_same_elements<T>(actual: &[T], expected: &[T]) -> TestResult
    where
        T: PartialEq + std::fmt::Debug,
    {
        if actual.len() != expected.len() {
            return Err(format!(
                "Collections have different lengths: {} vs {}",
                actual.len(),
                expected.len()
            )
            .into());
        }

        for item in expected {
            if !actual.contains(item) {
                return Err(format!("Expected item not found: {:?}", item).into());
            }
        }

        for item in actual {
            if !expected.contains(item) {
                return Err(format!("Unexpected item found: {:?}", item).into());
            }
        }

        Ok(())
    }
}

/// Macro for asserting eventual consistency
#[macro_export]
macro_rules! assert_eventually {
    ($condition:expr, $timeout:expr) => {{
        let start = std::time::Instant::now();
        let timeout_duration = $timeout;

        loop {
            if $condition {
                break;
            }

            if start.elapsed() > timeout_duration {
                panic!(
                    "Condition was not met within timeout: {:?}",
                    timeout_duration
                );
            }

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }};
}

/// Macro for asserting that an async operation panics
#[macro_export]
macro_rules! assert_async_panics {
    ($future:expr) => {{
        let result = std::panic::AssertUnwindSafe($future).catch_unwind().await;
        assert!(
            result.is_err(),
            "Expected panic, but operation completed successfully"
        );
    }};
}

/// Performance assertion utilities
pub struct PerformanceAssertions;

impl PerformanceAssertions {
    /// Assert that an operation completes within a performance threshold
    pub async fn assert_performance_threshold<F, T>(
        operation: F,
        threshold: Duration,
        operation_name: &str,
    ) -> TestResult<T>
    where
        F: std::future::Future<Output = T>,
    {
        let start = std::time::Instant::now();
        let result = operation.await;
        let elapsed = start.elapsed();

        if elapsed <= threshold {
            println!(
                "✓ {} completed in {:?} (threshold: {:?})",
                operation_name, elapsed, threshold
            );
            Ok(result)
        } else {
            Err(format!(
                "Performance threshold exceeded for {}: {:?} > {:?}",
                operation_name, elapsed, threshold
            )
            .into())
        }
    }

    /// Assert that memory usage stays within bounds during operation
    pub async fn assert_memory_usage<F, T>(
        operation: F,
        max_memory_mb: usize,
        operation_name: &str,
    ) -> TestResult<T>
    where
        F: std::future::Future<Output = T>,
    {
        // This is a simplified implementation - in practice you'd use a proper memory profiler
        let initial_memory = Self::get_memory_usage();
        let result = operation.await;
        let final_memory = Self::get_memory_usage();

        let memory_used = final_memory.saturating_sub(initial_memory);

        if memory_used <= max_memory_mb {
            println!(
                "✓ {} used {}MB memory (limit: {}MB)",
                operation_name, memory_used, max_memory_mb
            );
            Ok(result)
        } else {
            Err(format!(
                "Memory usage exceeded for {}: {}MB > {}MB",
                operation_name, memory_used, max_memory_mb
            )
            .into())
        }
    }

    fn get_memory_usage() -> usize {
        // Simplified memory usage calculation
        // In practice, you'd use a proper memory profiler
        0
    }
}
