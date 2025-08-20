//! Message Classification System for QoS-Aware Routing
//!
//! Implements intelligent message classification using machine learning techniques,
//! pattern recognition, and adaptive learning for optimal QoS determination.

use chrono::{DateTime, Datelike, Timelike, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

use crate::core::networking::valkyrie::adapters::*;
use crate::error::{Result, ValkyrieError};

pub use super::QoSClass;

/// Advanced message classifier with ML capabilities
pub struct MessageClassifier {
    /// Static classification rules
    static_rules: Vec<ClassificationRule>,
    /// Dynamic learning engine
    learning_engine: Arc<LearningEngine>,
    /// Pattern recognition system
    pattern_recognizer: Arc<PatternRecognizer>,
    /// Classification cache for performance
    classification_cache: Arc<RwLock<HashMap<String, CachedClassification>>>,
    /// Performance metrics
    metrics: Arc<RwLock<ClassificationMetrics>>,
    /// Configuration
    config: ClassifierConfig,
}

/// Learning engine for adaptive classification
pub struct LearningEngine {
    /// Training data
    training_data: Arc<RwLock<Vec<TrainingExample>>>,
    /// Model weights
    model_weights: Arc<RwLock<HashMap<String, f64>>>,
    /// Learning rate
    learning_rate: f64,
    /// Model accuracy
    accuracy: Arc<RwLock<f64>>,
}

/// Pattern recognition system
pub struct PatternRecognizer {
    /// Known patterns
    patterns: Arc<RwLock<Vec<MessagePattern>>>,
    /// Pattern matching cache
    pattern_cache: Arc<RwLock<HashMap<String, Vec<PatternMatch>>>>,
    /// Pattern statistics
    pattern_stats: Arc<RwLock<HashMap<String, PatternStats>>>,
}

/// Classification rule
#[derive(Debug, Clone)]
pub struct ClassificationRule {
    /// Rule identifier
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule condition
    pub condition: ClassificationCondition,
    /// Target QoS class
    pub target_class: QoSClass,
    /// Rule weight (0.0 - 1.0)
    pub weight: f64,
    /// Rule priority (lower = higher priority)
    pub priority: u8,
    /// Rule enabled
    pub enabled: bool,
    /// Rule statistics
    pub stats: RuleStats,
}

/// Classification condition types
#[derive(Debug, Clone)]
pub enum ClassificationCondition {
    /// Message type condition
    MessageType(AdapterMessageType),
    /// Priority condition
    Priority(MessagePriority),
    /// Metadata condition
    Metadata {
        key: String,
        value: String,
        operator: ComparisonOperator,
    },
    /// Payload size condition
    PayloadSize {
        min: Option<usize>,
        max: Option<usize>,
    },
    /// Source adapter condition
    SourceAdapter(AdapterId),
    /// Destination condition
    Destination(String),
    /// Timestamp condition
    Timestamp {
        before: Option<DateTime<Utc>>,
        after: Option<DateTime<Utc>>,
    },
    /// Composite condition (AND/OR)
    Composite {
        operator: LogicalOperator,
        conditions: Vec<ClassificationCondition>,
    },
    /// Pattern-based condition
    Pattern(String),
    /// ML-based condition
    MachineLearning { model: String, threshold: f64 },
}

/// Comparison operators for conditions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonOperator {
    Equals,
    NotEquals,
    Contains,
    StartsWith,
    EndsWith,
    Regex(String),
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
}

/// Logical operators for composite conditions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicalOperator {
    And,
    Or,
    Not,
}

/// Cached classification result
#[derive(Debug, Clone)]
pub struct CachedClassification {
    /// QoS class
    pub qos_class: QoSClass,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Cache timestamp
    pub cached_at: Instant,
    /// Cache TTL
    pub ttl: Duration,
    /// Hit count
    pub hit_count: u32,
}

/// Training example for ML
#[derive(Debug, Clone)]
pub struct TrainingExample {
    /// Message features
    pub features: MessageFeatures,
    /// Correct QoS class
    pub correct_class: QoSClass,
    /// Timestamp
    pub timestamp: Instant,
    /// Feedback score
    pub feedback_score: f64,
}

/// Message features for ML classification
#[derive(Debug, Clone)]
pub struct MessageFeatures {
    /// Message type
    pub message_type: AdapterMessageType,
    /// Priority
    pub priority: MessagePriority,
    /// Payload size
    pub payload_size: usize,
    /// Metadata count
    pub metadata_count: usize,
    /// Source adapter type
    pub source_adapter_type: Option<AdapterType>,
    /// Timestamp features
    pub timestamp_features: TimestampFeatures,
    /// Content features
    pub content_features: ContentFeatures,
}

/// Timestamp-based features
#[derive(Debug, Clone)]
pub struct TimestampFeatures {
    /// Hour of day (0-23)
    pub hour_of_day: u8,
    /// Day of week (0-6)
    pub day_of_week: u8,
    /// Is weekend
    pub is_weekend: bool,
    /// Is business hours
    pub is_business_hours: bool,
}

/// Content-based features
#[derive(Debug, Clone)]
pub struct ContentFeatures {
    /// Contains keywords
    pub keywords: Vec<String>,
    /// Language detected
    pub language: Option<String>,
    /// Encoding type
    pub encoding: Option<String>,
    /// Compression ratio
    pub compression_ratio: Option<f64>,
}

/// Message pattern for recognition
#[derive(Debug, Clone)]
pub struct MessagePattern {
    /// Pattern ID
    pub id: String,
    /// Pattern name
    pub name: String,
    /// Pattern type
    pub pattern_type: PatternType,
    /// Pattern signature
    pub signature: PatternSignature,
    /// Associated QoS class
    pub qos_class: QoSClass,
    /// Pattern confidence
    pub confidence: f64,
    /// Pattern statistics
    pub stats: PatternStats,
}

/// Pattern types
#[derive(Debug, Clone)]
pub enum PatternType {
    /// Sequence pattern
    Sequence,
    /// Frequency pattern
    Frequency,
    /// Size pattern
    Size,
    /// Temporal pattern
    Temporal,
    /// Content pattern
    Content,
    /// Behavioral pattern
    Behavioral,
}

/// Pattern signature
#[derive(Debug, Clone)]
pub struct PatternSignature {
    /// Signature data
    pub data: Vec<u8>,
    /// Signature hash
    pub hash: String,
    /// Signature version
    pub version: u32,
}

/// Pattern match result
#[derive(Debug, Clone)]
pub struct PatternMatch {
    /// Pattern ID
    pub pattern_id: String,
    /// Match confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Match score
    pub score: f64,
    /// Match details
    pub details: HashMap<String, String>,
}

/// Pattern statistics
#[derive(Debug, Clone)]
pub struct PatternStats {
    /// Total matches
    pub total_matches: u64,
    /// Correct classifications
    pub correct_classifications: u64,
    /// Average confidence
    pub avg_confidence: f64,
    /// Last matched
    pub last_matched: Option<Instant>,
    /// Pattern accuracy
    pub accuracy: f64,
}

/// Rule statistics
#[derive(Debug, Clone)]
pub struct RuleStats {
    /// Total applications
    pub total_applications: u64,
    /// Successful applications
    pub successful_applications: u64,
    /// Average execution time
    pub avg_execution_time: Duration,
    /// Last applied
    pub last_applied: Option<Instant>,
    /// Rule accuracy
    pub accuracy: f64,
}

/// Classification metrics
#[derive(Debug, Clone)]
pub struct ClassificationMetrics {
    /// Total classifications
    pub total_classifications: u64,
    /// Classifications by class
    pub by_class: HashMap<QoSClass, u64>,
    /// Average classification time
    pub avg_classification_time: Duration,
    /// Cache hit rate
    pub cache_hit_rate: f64,
    /// ML accuracy
    pub ml_accuracy: f64,
    /// Pattern recognition accuracy
    pub pattern_accuracy: f64,
    /// Misclassification rate
    pub misclassification_rate: f64,
}

/// Classifier configuration
#[derive(Debug, Clone)]
pub struct ClassifierConfig {
    /// Enable ML classification
    pub enable_ml: bool,
    /// Enable pattern recognition
    pub enable_patterns: bool,
    /// Enable caching
    pub enable_caching: bool,
    /// Cache TTL
    pub cache_ttl: Duration,
    /// Max cache size
    pub max_cache_size: usize,
    /// Learning rate for ML
    pub learning_rate: f64,
    /// Pattern confidence threshold
    pub pattern_threshold: f64,
    /// ML confidence threshold
    pub ml_threshold: f64,
    /// Enable adaptive learning
    pub adaptive_learning: bool,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            enable_ml: true,
            enable_patterns: true,
            enable_caching: true,
            cache_ttl: Duration::from_secs(300), // 5 minutes
            max_cache_size: 10000,
            learning_rate: 0.01,
            pattern_threshold: 0.7,
            ml_threshold: 0.8,
            adaptive_learning: true,
        }
    }
}

impl MessageClassifier {
    /// Create new message classifier
    pub fn new() -> Self {
        Self::with_config(ClassifierConfig::default())
    }

    /// Create classifier with custom configuration
    pub fn with_config(config: ClassifierConfig) -> Self {
        Self {
            static_rules: Self::default_rules(),
            learning_engine: Arc::new(LearningEngine::new(config.learning_rate)),
            pattern_recognizer: Arc::new(PatternRecognizer::new()),
            classification_cache: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(ClassificationMetrics::default())),
            config,
        }
    }

    /// Classify message with advanced techniques
    pub async fn classify(&self, message: &AdapterMessage, qos: &QoSParams) -> Result<QoSClass> {
        let start_time = Instant::now();
        let message_hash = self.calculate_message_hash(message);

        // Check cache first
        if self.config.enable_caching {
            if let Some(cached) = self.check_cache(&message_hash).await {
                self.update_metrics_cache_hit(start_time.elapsed()).await;
                return Ok(cached.qos_class);
            }
        }

        // Extract message features
        let features = self.extract_features(message, qos).await?;

        // Try different classification methods in order of confidence
        let mut classification_results = Vec::new();

        // 1. Static rules classification
        if let Some(rule_result) = self.classify_with_rules(message, qos, &features).await? {
            classification_results.push(rule_result);
        }

        // 2. Pattern recognition
        if self.config.enable_patterns {
            if let Some(pattern_result) = self.classify_with_patterns(message, &features).await? {
                classification_results.push(pattern_result);
            }
        }

        // 3. Machine learning
        if self.config.enable_ml {
            if let Some(ml_result) = self.classify_with_ml(&features).await? {
                classification_results.push(ml_result);
            }
        }

        // Combine results using weighted voting
        let final_result = self
            .combine_classification_results(classification_results)
            .await?;

        // Cache the result
        if self.config.enable_caching {
            self.cache_classification(&message_hash, &final_result)
                .await;
        }

        // Update metrics
        self.update_metrics(final_result.qos_class, start_time.elapsed())
            .await;

        // Store for learning if adaptive learning is enabled
        if self.config.adaptive_learning {
            self.store_for_learning(features, final_result.qos_class)
                .await;
        }

        Ok(final_result.qos_class)
    }

    /// Classify using static rules
    async fn classify_with_rules(
        &self,
        message: &AdapterMessage,
        qos: &QoSParams,
        features: &MessageFeatures,
    ) -> Result<Option<ClassificationResult>> {
        for rule in &self.static_rules {
            if !rule.enabled {
                continue;
            }

            if self
                .evaluate_condition(&rule.condition, message, qos, features)
                .await?
            {
                return Ok(Some(ClassificationResult {
                    qos_class: rule.target_class,
                    confidence: rule.weight,
                    method: ClassificationMethod::Rules,
                    details: format!("Matched rule: {}", rule.name),
                }));
            }
        }

        Ok(None)
    }

    /// Classify using pattern recognition
    async fn classify_with_patterns(
        &self,
        message: &AdapterMessage,
        features: &MessageFeatures,
    ) -> Result<Option<ClassificationResult>> {
        let matches = self
            .pattern_recognizer
            .find_matches(message, features)
            .await?;

        if let Some(best_match) = matches
            .into_iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
        {
            if best_match.confidence >= self.config.pattern_threshold {
                let patterns = self.pattern_recognizer.patterns.read().await;
                if let Some(pattern) = patterns.iter().find(|p| p.id == best_match.pattern_id) {
                    return Ok(Some(ClassificationResult {
                        qos_class: pattern.qos_class,
                        confidence: best_match.confidence,
                        method: ClassificationMethod::Patterns,
                        details: format!("Matched pattern: {}", pattern.name),
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Classify using machine learning
    async fn classify_with_ml(
        &self,
        features: &MessageFeatures,
    ) -> Result<Option<ClassificationResult>> {
        let prediction = self.learning_engine.predict(features).await?;

        if prediction.confidence >= self.config.ml_threshold {
            return Ok(Some(ClassificationResult {
                qos_class: prediction.qos_class,
                confidence: prediction.confidence,
                method: ClassificationMethod::MachineLearning,
                details: format!(
                    "ML prediction with confidence: {:.2}",
                    prediction.confidence
                ),
            }));
        }

        Ok(None)
    }

    /// Combine multiple classification results
    async fn combine_classification_results(
        &self,
        results: Vec<ClassificationResult>,
    ) -> Result<ClassificationResult> {
        if results.is_empty() {
            return Ok(ClassificationResult {
                qos_class: QoSClass::DataTransfer, // Default
                confidence: 0.5,
                method: ClassificationMethod::Default,
                details: "No classification method matched".to_string(),
            });
        }

        // Weighted voting based on confidence and method priority
        let mut class_scores: HashMap<QoSClass, f64> = HashMap::new();

        for result in &results {
            let method_weight = match result.method {
                ClassificationMethod::Rules => 1.0,
                ClassificationMethod::MachineLearning => 0.9,
                ClassificationMethod::Patterns => 0.8,
                ClassificationMethod::Default => 0.1,
                ClassificationMethod::Combined => 0.8,
            };

            let weighted_confidence = result.confidence * method_weight;
            *class_scores.entry(result.qos_class).or_insert(0.0) += weighted_confidence;
        }

        // Find the class with highest score
        let (best_class, best_score) = class_scores
            .into_iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        Ok(ClassificationResult {
            qos_class: best_class,
            confidence: best_score / results.len() as f64,
            method: ClassificationMethod::Combined,
            details: format!("Combined {} methods", results.len()),
        })
    }

    /// Extract features from message
    async fn extract_features(
        &self,
        message: &AdapterMessage,
        qos: &QoSParams,
    ) -> Result<MessageFeatures> {
        let now = Utc::now();

        Ok(MessageFeatures {
            message_type: message.message_type.clone(),
            priority: message.priority,
            payload_size: message.payload.len(),
            metadata_count: message.metadata.len(),
            source_adapter_type: None, // Would be extracted from routing info
            timestamp_features: TimestampFeatures {
                hour_of_day: now.hour() as u8,
                day_of_week: now.weekday().num_days_from_monday() as u8,
                is_weekend: matches!(now.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun),
                is_business_hours: (9..17).contains(&now.hour()),
            },
            content_features: ContentFeatures {
                keywords: self.extract_keywords(&message.payload),
                language: None,          // Would use language detection
                encoding: None,          // Would detect encoding
                compression_ratio: None, // Would calculate if compressed
            },
        })
    }

    /// Extract keywords from payload
    fn extract_keywords(&self, payload: &[u8]) -> Vec<String> {
        // Simple keyword extraction - in practice would use NLP
        if let Ok(text) = std::str::from_utf8(payload) {
            text.split_whitespace()
                .filter(|word| word.len() > 3)
                .take(10)
                .map(|s| s.to_lowercase())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Calculate message hash for caching
    fn calculate_message_hash(&self, message: &AdapterMessage) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        message.message_type.hash(&mut hasher);
        message.priority.hash(&mut hasher);
        message.payload.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }

    /// Check classification cache
    async fn check_cache(&self, message_hash: &str) -> Option<CachedClassification> {
        let cache = self.classification_cache.read().await;
        if let Some(cached) = cache.get(message_hash) {
            if cached.cached_at.elapsed() < cached.ttl {
                return Some(cached.clone());
            }
        }
        None
    }

    /// Cache classification result
    async fn cache_classification(&self, message_hash: &str, result: &ClassificationResult) {
        let mut cache = self.classification_cache.write().await;

        // Cleanup old entries if cache is full
        if cache.len() >= self.config.max_cache_size {
            let oldest_key = cache
                .iter()
                .min_by_key(|(_, v)| v.cached_at)
                .map(|(k, _)| k.clone());

            if let Some(key) = oldest_key {
                cache.remove(&key);
            }
        }

        cache.insert(
            message_hash.to_string(),
            CachedClassification {
                qos_class: result.qos_class,
                confidence: result.confidence,
                cached_at: Instant::now(),
                ttl: self.config.cache_ttl,
                hit_count: 0,
            },
        );
    }

    /// Evaluate classification condition
    fn evaluate_condition<'a>(
        &'a self,
        condition: &'a ClassificationCondition,
        message: &'a AdapterMessage,
        qos: &'a QoSParams,
        features: &'a MessageFeatures,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move {
            match condition {
                ClassificationCondition::MessageType(msg_type) => {
                    Ok(message.message_type == *msg_type)
                }
                ClassificationCondition::Priority(priority) => Ok(message.priority == *priority),
                ClassificationCondition::Metadata {
                    key,
                    value,
                    operator,
                } => {
                    if let Some(msg_value) = message.metadata.get(key) {
                        Ok(self.compare_values(msg_value, value, operator))
                    } else {
                        Ok(false)
                    }
                }
                ClassificationCondition::PayloadSize { min, max } => {
                    let size = message.payload.len();
                    Ok(min.map_or(true, |m| size >= m) && max.map_or(true, |m| size <= m))
                }
                ClassificationCondition::Composite {
                    operator,
                    conditions,
                } => {
                    self.evaluate_composite_condition(operator, conditions, message, qos, features)
                        .await
                }
                _ => Ok(false), // Placeholder for other conditions
            }
        })
    }

    /// Evaluate composite condition
    async fn evaluate_composite_condition(
        &self,
        operator: &LogicalOperator,
        conditions: &[ClassificationCondition],
        message: &AdapterMessage,
        qos: &QoSParams,
        features: &MessageFeatures,
    ) -> Result<bool> {
        match operator {
            LogicalOperator::And => {
                for condition in conditions {
                    if !self
                        .evaluate_condition(condition, message, qos, features)
                        .await?
                    {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            LogicalOperator::Or => {
                for condition in conditions {
                    if self
                        .evaluate_condition(condition, message, qos, features)
                        .await?
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            LogicalOperator::Not => {
                if conditions.len() != 1 {
                    return Err(ValkyrieError::InvalidCondition(
                        "NOT operator requires exactly one condition".to_string(),
                    ));
                }
                let result = self
                    .evaluate_condition(&conditions[0], message, qos, features)
                    .await?;
                Ok(!result)
            }
        }
    }

    /// Compare values using operator
    fn compare_values(&self, left: &str, right: &str, operator: &ComparisonOperator) -> bool {
        match operator {
            ComparisonOperator::Equals => left == right,
            ComparisonOperator::NotEquals => left != right,
            ComparisonOperator::Contains => left.contains(right),
            ComparisonOperator::StartsWith => left.starts_with(right),
            ComparisonOperator::EndsWith => left.ends_with(right),
            ComparisonOperator::Regex(pattern) => {
                // Would use regex crate in practice
                left.contains(pattern) // Simplified
            }
            _ => false, // Numeric comparisons would need type conversion
        }
    }

    /// Store classification for learning
    async fn store_for_learning(&self, features: MessageFeatures, qos_class: QoSClass) {
        self.learning_engine
            .add_training_example(TrainingExample {
                features,
                correct_class: qos_class,
                timestamp: Instant::now(),
                feedback_score: 1.0, // Would be updated based on feedback
            })
            .await;
    }

    /// Update classification metrics
    async fn update_metrics(&self, qos_class: QoSClass, latency: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.total_classifications += 1;
        *metrics.by_class.entry(qos_class).or_insert(0) += 1;

        // Update average classification time
        let total_time = metrics.avg_classification_time.as_nanos()
            * (metrics.total_classifications - 1) as u128
            + latency.as_nanos();
        metrics.avg_classification_time =
            Duration::from_nanos((total_time / metrics.total_classifications as u128) as u64);
    }

    /// Update cache hit metrics
    async fn update_metrics_cache_hit(&self, latency: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.total_classifications += 1;

        // Update cache hit rate
        let cache_hits =
            (metrics.cache_hit_rate * (metrics.total_classifications - 1) as f64) + 1.0;
        metrics.cache_hit_rate = cache_hits / metrics.total_classifications as f64;

        // Update average time
        let total_time = metrics.avg_classification_time.as_nanos()
            * (metrics.total_classifications - 1) as u128
            + latency.as_nanos();
        metrics.avg_classification_time =
            Duration::from_nanos((total_time / metrics.total_classifications as u128) as u64);
    }

    /// Default classification rules
    fn default_rules() -> Vec<ClassificationRule> {
        vec![
            ClassificationRule {
                id: "critical-health".to_string(),
                name: "Critical Health Checks".to_string(),
                condition: ClassificationCondition::MessageType(AdapterMessageType::Heartbeat),
                target_class: QoSClass::Critical,
                weight: 1.0,
                priority: 0,
                enabled: true,
                stats: RuleStats::default(),
            },
            ClassificationRule {
                id: "high-priority".to_string(),
                name: "High Priority Messages".to_string(),
                condition: ClassificationCondition::Priority(MessagePriority::Critical),
                target_class: QoSClass::System,
                weight: 0.9,
                priority: 1,
                enabled: true,
                stats: RuleStats::default(),
            },
            ClassificationRule {
                id: "job-execution".to_string(),
                name: "Job Execution Commands".to_string(),
                condition: ClassificationCondition::MessageType(AdapterMessageType::Request),
                target_class: QoSClass::JobExecution,
                weight: 0.8,
                priority: 2,
                enabled: true,
                stats: RuleStats::default(),
            },
            ClassificationRule {
                id: "large-data".to_string(),
                name: "Large Data Transfers".to_string(),
                condition: ClassificationCondition::PayloadSize {
                    min: Some(1024 * 1024), // 1MB
                    max: None,
                },
                target_class: QoSClass::DataTransfer,
                weight: 0.7,
                priority: 3,
                enabled: true,
                stats: RuleStats::default(),
            },
        ]
    }

    /// Get classification metrics
    pub async fn metrics(&self) -> ClassificationMetrics {
        self.metrics.read().await.clone()
    }
}

/// Classification result
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    /// Determined QoS class
    pub qos_class: QoSClass,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Classification method used
    pub method: ClassificationMethod,
    /// Additional details
    pub details: String,
}

/// Classification methods
#[derive(Debug, Clone)]
pub enum ClassificationMethod {
    Rules,
    MachineLearning,
    Patterns,
    Combined,
    Default,
}

/// ML prediction result
#[derive(Debug, Clone)]
pub struct MLPrediction {
    /// Predicted QoS class
    pub qos_class: QoSClass,
    /// Prediction confidence
    pub confidence: f64,
}

impl LearningEngine {
    /// Create new learning engine
    pub fn new(learning_rate: f64) -> Self {
        Self {
            training_data: Arc::new(RwLock::new(Vec::new())),
            model_weights: Arc::new(RwLock::new(HashMap::new())),
            learning_rate,
            accuracy: Arc::new(RwLock::new(0.0)),
        }
    }

    /// Add training example
    pub async fn add_training_example(&self, example: TrainingExample) {
        let mut data = self.training_data.write().await;
        data.push(example);

        // Trigger retraining if we have enough examples
        if data.len() % 100 == 0 {
            self.retrain().await;
        }
    }

    /// Predict QoS class using ML model
    pub async fn predict(&self, features: &MessageFeatures) -> Result<MLPrediction> {
        let weights = self.model_weights.read().await;

        // Simple linear model for demonstration
        let mut scores: HashMap<QoSClass, f64> = HashMap::new();

        // Calculate scores for each QoS class
        for qos_class in [
            QoSClass::Critical,
            QoSClass::System,
            QoSClass::JobExecution,
            QoSClass::DataTransfer,
            QoSClass::LogsMetrics,
        ] {
            let class_key = format!("{:?}", qos_class);
            let weight = weights.get(&class_key).unwrap_or(&0.0);

            // Simple feature scoring
            let mut score = *weight;
            score += match features.priority {
                MessagePriority::Critical => 0.9,
                MessagePriority::High => 0.7,
                MessagePriority::Normal => 0.5,
                MessagePriority::Low => 0.3,
                MessagePriority::Background => 0.1,
            };

            score += (features.payload_size as f64).log10() * 0.1;

            scores.insert(qos_class, score);
        }

        // Find best prediction
        let (best_class, best_score) = scores
            .into_iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap_or((QoSClass::DataTransfer, 0.5));

        // Normalize confidence to 0-1 range
        let confidence = (best_score.tanh() + 1.0) / 2.0;

        Ok(MLPrediction {
            qos_class: best_class,
            confidence,
        })
    }

    /// Retrain the model
    async fn retrain(&self) {
        let data = self.training_data.read().await;
        let mut weights = self.model_weights.write().await;

        // Simple gradient descent training
        for example in data.iter() {
            let prediction = self.predict_internal(&example.features, &weights);
            let error = if prediction.qos_class == example.correct_class {
                0.0
            } else {
                1.0
            };

            // Update weights based on error
            let class_key = format!("{:?}", example.correct_class);
            let current_weight = weights.get(&class_key).unwrap_or(&0.0);
            let new_weight = current_weight + (self.learning_rate * error);
            weights.insert(class_key, new_weight);
        }

        info!("Retrained ML model with {} examples", data.len());
    }

    /// Internal prediction for training
    fn predict_internal(
        &self,
        features: &MessageFeatures,
        weights: &HashMap<String, f64>,
    ) -> MLPrediction {
        // Simplified prediction logic
        MLPrediction {
            qos_class: QoSClass::DataTransfer,
            confidence: 0.5,
        }
    }
}

impl PatternRecognizer {
    /// Create new pattern recognizer
    pub fn new() -> Self {
        Self {
            patterns: Arc::new(RwLock::new(Vec::new())),
            pattern_cache: Arc::new(RwLock::new(HashMap::new())),
            pattern_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Find pattern matches for message
    pub async fn find_matches(
        &self,
        message: &AdapterMessage,
        features: &MessageFeatures,
    ) -> Result<Vec<PatternMatch>> {
        let patterns = self.patterns.read().await;
        let mut matches = Vec::new();

        for pattern in patterns.iter() {
            if let Some(match_result) = self.match_pattern(pattern, message, features).await {
                matches.push(match_result);
            }
        }

        Ok(matches)
    }

    /// Match single pattern
    async fn match_pattern(
        &self,
        pattern: &MessagePattern,
        message: &AdapterMessage,
        features: &MessageFeatures,
    ) -> Option<PatternMatch> {
        // Simplified pattern matching
        let confidence = match pattern.pattern_type {
            PatternType::Size => {
                let size_ratio = features.payload_size as f64 / 1024.0; // Normalize to KB
                if size_ratio > 1000.0 {
                    0.9
                } else {
                    0.3
                }
            }
            PatternType::Frequency => 0.5, // Would analyze message frequency
            PatternType::Content => 0.6,   // Would analyze content patterns
            _ => 0.4,
        };

        if confidence > 0.5 {
            Some(PatternMatch {
                pattern_id: pattern.id.clone(),
                confidence,
                score: confidence * pattern.confidence,
                details: HashMap::new(),
            })
        } else {
            None
        }
    }
}

impl Default for ClassificationMetrics {
    fn default() -> Self {
        Self {
            total_classifications: 0,
            by_class: HashMap::new(),
            avg_classification_time: Duration::from_nanos(0),
            cache_hit_rate: 0.0,
            ml_accuracy: 0.0,
            pattern_accuracy: 0.0,
            misclassification_rate: 0.0,
        }
    }
}

impl Default for RuleStats {
    fn default() -> Self {
        Self {
            total_applications: 0,
            successful_applications: 0,
            avg_execution_time: Duration::from_nanos(0),
            last_applied: None,
            accuracy: 0.0,
        }
    }
}

impl Default for PatternStats {
    fn default() -> Self {
        Self {
            total_matches: 0,
            correct_classifications: 0,
            avg_confidence: 0.0,
            last_matched: None,
            accuracy: 0.0,
        }
    }
}
