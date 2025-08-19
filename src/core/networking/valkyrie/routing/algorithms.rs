// Routing Algorithm Implementations
// Task 3.1.1.2 & 3.1.1.3: Shortest Path and Load-Aware Algorithms

use super::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Wrapper for f64 to make it orderable in BinaryHeap
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrderedFloat(pub f64);

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

/// Dijkstra's shortest path algorithm implementation
pub struct DijkstraStrategy {
    cost_calculator: Arc<dyn CostCalculator>,
}

/// Load-aware routing using A* with dynamic cost adjustment
pub struct LoadAwareStrategy {
    cost_calculator: Arc<dyn CostCalculator>,
    load_monitor: Arc<dyn LoadMonitor>,
    heuristic_calculator: Arc<dyn HeuristicCalculator>,
}

/// Latency-optimized routing strategy
pub struct LatencyOptimizedStrategy {
    latency_predictor: Arc<dyn LatencyPredictor>,
}

/// Cost-optimized routing strategy
pub struct CostOptimizedStrategy {
    cost_model: Arc<dyn CostModel>,
}

/// Adaptive routing strategy that learns from historical data
pub struct AdaptiveStrategy {
    ml_model: Arc<dyn MLModel>,
    feature_extractor: Arc<dyn FeatureExtractor>,
    fallback_strategy: Arc<dyn RoutingStrategy>,
}

/// Trait for calculating link costs
#[async_trait::async_trait]
pub trait CostCalculator: Send + Sync {
    async fn calculate_cost(
        &self,
        link: &NetworkLink,
        context: &RoutingContext,
        current_load: Option<f64>,
    ) -> f64;
}

/// Trait for monitoring network load
#[async_trait::async_trait]
pub trait LoadMonitor: Send + Sync {
    async fn get_link_load(&self, link_id: &LinkId) -> Option<f64>;
    async fn get_node_load(&self, node_id: &NodeId) -> Option<f64>;
    async fn predict_load(&self, link_id: &LinkId, duration: Duration) -> Option<f64>;
}

/// Trait for calculating heuristic costs (A* algorithm)
pub trait HeuristicCalculator: Send + Sync {
    fn calculate_heuristic(
        &self,
        from: &NodeId,
        to: &NodeId,
        topology: &NetworkTopology,
        context: &RoutingContext,
    ) -> f64;
}

/// Trait for predicting latency
#[async_trait::async_trait]
pub trait LatencyPredictor: Send + Sync {
    async fn predict_latency(
        &self,
        from: &NodeId,
        to: &NodeId,
        context: &RoutingContext,
    ) -> Duration;
}

/// Trait for cost modeling
pub trait CostModel: Send + Sync {
    fn calculate_total_cost(
        &self,
        route: &[NodeId],
        topology: &NetworkTopology,
        context: &RoutingContext,
    ) -> f64;
}

/// Trait for machine learning models
#[async_trait::async_trait]
pub trait MLModel: Send + Sync {
    async fn predict_route_quality(&self, route: &Route, context: &RoutingContext) -> f64;

    async fn train(&mut self, training_data: &[TrainingExample]);
}

/// Training example for ML models
#[derive(Debug, Clone)]
pub struct TrainingExample {
    pub route: Route,
    pub context: RoutingContext,
    pub actual_performance: PerformanceMetrics,
}

/// Performance metrics for routes
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub actual_latency: Duration,
    pub throughput: u64,
    pub error_rate: f64,
    pub availability: f64,
}

/// Feature extraction for ML models
pub trait FeatureExtractor: Send + Sync {
    fn extract_features(
        &self,
        route: &Route,
        context: &RoutingContext,
        topology: &NetworkTopology,
    ) -> FeatureVector;
}

/// Feature vector for ML models
#[derive(Debug, Clone)]
pub struct FeatureVector {
    pub features: Vec<f64>,
    pub feature_names: Vec<String>,
}

// Implementation of Dijkstra Strategy
impl DijkstraStrategy {
    pub fn new(cost_calculator: Arc<dyn CostCalculator>) -> Self {
        Self { cost_calculator }
    }

    async fn reconstruct_path(
        &self,
        source: &NodeId,
        destination: &NodeId,
        previous: &HashMap<NodeId, NodeId>,
        topology: &NetworkTopology,
    ) -> Result<Route, RoutingError> {
        let mut path = Vec::new();
        let mut current = *destination;

        // Reconstruct path from destination to source
        while current != *source {
            path.push(current);
            current = *previous
                .get(&current)
                .ok_or_else(|| RoutingError::NoRouteFound {
                    source: *source,
                    destination: *destination,
                })?;
        }
        path.push(*source);
        path.reverse();

        // Build hops from path
        let mut hops = Vec::new();
        let mut total_cost = 0.0;
        let mut total_latency = Duration::ZERO;
        let mut min_bandwidth = u64::MAX;
        let mut reliability_product = 1.0;

        for window in path.windows(2) {
            let from = window[0];
            let to = window[1];

            // Find the link between these nodes
            let link = topology
                .links
                .values()
                .find(|link| link.from == from && link.to == to)
                .ok_or_else(|| RoutingError::Internal {
                    message: format!("Link not found between {} and {}", from, to),
                })?;

            let hop = Hop {
                from,
                to,
                transport: link.transport,
                cost: link.cost,
                latency: link.latency,
                bandwidth: link.bandwidth,
                reliability: link.reliability,
                security_level: topology
                    .nodes
                    .get(&to)
                    .map(|n| n.security_level)
                    .unwrap_or(SecurityLevel::Internal),
            };

            total_cost += hop.cost;
            total_latency += hop.latency;
            min_bandwidth = min_bandwidth.min(hop.bandwidth);
            reliability_product *= hop.reliability;

            hops.push(hop);
        }

        Ok(Route {
            id: Uuid::new_v4(),
            path,
            hops,
            total_cost,
            estimated_latency: total_latency,
            reliability_score: reliability_product,
            bandwidth_available: min_bandwidth,
            created_at: SystemTime::now(),
            expires_at: Some(SystemTime::now() + Duration::from_secs(300)), // 5 minute TTL
            metadata: RouteMetadata::default(),
        })
    }
}

#[async_trait::async_trait]
impl RoutingStrategy for DijkstraStrategy {
    async fn calculate_route(
        &self,
        source: &NodeId,
        destination: &NodeId,
        context: &RoutingContext,
        topology: &NetworkTopology,
    ) -> Result<Route, RoutingError> {
        let start_time = Instant::now();

        // Check if source and destination exist
        if !topology.nodes.contains_key(source) {
            return Err(RoutingError::InvalidContext {
                reason: format!("Source node {} not found", source),
            });
        }
        if !topology.nodes.contains_key(destination) {
            return Err(RoutingError::InvalidContext {
                reason: format!("Destination node {} not found", destination),
            });
        }

        // If source equals destination, return empty route
        if source == destination {
            return Ok(Route {
                id: Uuid::new_v4(),
                path: vec![*source],
                hops: Vec::new(),
                total_cost: 0.0,
                estimated_latency: Duration::ZERO,
                reliability_score: 1.0,
                bandwidth_available: u64::MAX,
                created_at: SystemTime::now(),
                expires_at: Some(SystemTime::now() + Duration::from_secs(300)),
                metadata: RouteMetadata {
                    algorithm_used: "Dijkstra".to_string(),
                    calculation_time: start_time.elapsed(),
                    cache_hit: false,
                    alternative_routes: 0,
                    confidence_score: 1.0,
                },
            });
        }

        let mut distances: HashMap<NodeId, f64> = HashMap::new();
        let mut previous: HashMap<NodeId, NodeId> = HashMap::new();
        let mut unvisited = BinaryHeap::new();
        let mut visited = HashSet::new();

        // Initialize distances
        for node_id in topology.nodes.keys() {
            let distance = if node_id == source {
                0.0
            } else {
                f64::INFINITY
            };
            distances.insert(*node_id, distance);
            unvisited.push((std::cmp::Reverse(OrderedFloat(distance)), *node_id));
        }

        while let Some((std::cmp::Reverse(OrderedFloat(current_distance)), current_node)) =
            unvisited.pop()
        {
            // Skip if we've already visited this node
            if visited.contains(&current_node) {
                continue;
            }

            visited.insert(current_node);

            // If we reached the destination, we're done
            if current_node == *destination {
                break;
            }

            // Skip if current distance is greater than recorded (stale entry)
            if current_distance > distances[&current_node] {
                continue;
            }

            // Check timeout
            if start_time.elapsed() > Duration::from_millis(100) {
                return Err(RoutingError::CalculationTimeout {
                    duration: start_time.elapsed(),
                });
            }

            // Examine neighbors
            for link in topology.get_outgoing_links(&current_node) {
                let neighbor = link.to;

                // Skip if already visited
                if visited.contains(&neighbor) {
                    continue;
                }

                // Calculate cost for this link
                let link_cost = self
                    .cost_calculator
                    .calculate_cost(link, context, None)
                    .await;

                let alt_distance = distances[&current_node] + link_cost;

                if alt_distance < distances[&neighbor] {
                    distances.insert(neighbor, alt_distance);
                    previous.insert(neighbor, current_node);
                    unvisited.push((std::cmp::Reverse(OrderedFloat(alt_distance)), neighbor));
                }
            }
        }

        // Check if destination is reachable
        if !previous.contains_key(destination) && source != destination {
            return Err(RoutingError::NoRouteFound {
                source: *source,
                destination: *destination,
            });
        }

        self.reconstruct_path(source, destination, &previous, topology)
            .await
    }

    fn get_algorithm_type(&self) -> RoutingAlgorithm {
        RoutingAlgorithm::ShortestPath
    }

    fn get_priority(&self) -> u8 {
        100 // High priority, reliable algorithm
    }

    fn can_handle(&self, _context: &RoutingContext) -> bool {
        true // Can handle any context
    }

    fn get_performance_profile(&self) -> PerformanceProfile {
        PerformanceProfile {
            avg_calculation_time: Duration::from_micros(500),
            memory_usage: 1024 * 1024, // 1MB
            cpu_intensity: 0.7,
            scalability_factor: 0.8, // O(V log V + E)
        }
    }
}

// Implementation of Load-Aware Strategy
impl LoadAwareStrategy {
    pub fn new(
        cost_calculator: Arc<dyn CostCalculator>,
        load_monitor: Arc<dyn LoadMonitor>,
        heuristic_calculator: Arc<dyn HeuristicCalculator>,
    ) -> Self {
        Self {
            cost_calculator,
            load_monitor,
            heuristic_calculator,
        }
    }
}

#[async_trait::async_trait]
impl RoutingStrategy for LoadAwareStrategy {
    async fn calculate_route(
        &self,
        source: &NodeId,
        destination: &NodeId,
        context: &RoutingContext,
        topology: &NetworkTopology,
    ) -> Result<Route, RoutingError> {
        let start_time = Instant::now();

        // A* algorithm with load-aware costs
        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<NodeId, NodeId> = HashMap::new();
        let mut g_score: HashMap<NodeId, f64> = HashMap::new();
        let mut f_score: HashMap<NodeId, f64> = HashMap::new();
        let mut closed_set = HashSet::new();

        // Initialize scores
        g_score.insert(*source, 0.0);
        let h_score =
            self.heuristic_calculator
                .calculate_heuristic(source, destination, topology, context);
        f_score.insert(*source, h_score);

        open_set.push((std::cmp::Reverse(OrderedFloat(h_score)), *source));

        while let Some((_, current)) = open_set.pop() {
            // Skip if already processed
            if closed_set.contains(&current) {
                continue;
            }

            closed_set.insert(current);

            // Check if we reached the destination
            if current == *destination {
                return self
                    .reconstruct_astar_path(source, destination, &came_from, topology)
                    .await;
            }

            // Check timeout
            if start_time.elapsed() > Duration::from_millis(150) {
                return Err(RoutingError::CalculationTimeout {
                    duration: start_time.elapsed(),
                });
            }

            // Examine neighbors
            for link in topology.get_outgoing_links(&current) {
                let neighbor = link.to;

                if closed_set.contains(&neighbor) {
                    continue;
                }

                // Get current load for this link
                let current_load = self.load_monitor.get_link_load(&link.id).await;

                // Calculate load-aware cost
                let link_cost = self
                    .cost_calculator
                    .calculate_cost(link, context, current_load)
                    .await;

                let tentative_g_score = g_score[&current] + link_cost;

                if tentative_g_score < *g_score.get(&neighbor).unwrap_or(&f64::INFINITY) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g_score);

                    let h_score = self.heuristic_calculator.calculate_heuristic(
                        &neighbor,
                        destination,
                        topology,
                        context,
                    );
                    let f = tentative_g_score + h_score;
                    f_score.insert(neighbor, f);

                    open_set.push((std::cmp::Reverse(OrderedFloat(f)), neighbor));
                }
            }
        }

        Err(RoutingError::NoRouteFound {
            source: *source,
            destination: *destination,
        })
    }

    fn get_algorithm_type(&self) -> RoutingAlgorithm {
        RoutingAlgorithm::LoadAware
    }

    fn get_priority(&self) -> u8 {
        120 // Higher priority than Dijkstra due to load awareness
    }

    fn can_handle(&self, _context: &RoutingContext) -> bool {
        true
    }

    fn get_performance_profile(&self) -> PerformanceProfile {
        PerformanceProfile {
            avg_calculation_time: Duration::from_micros(800),
            memory_usage: 1536 * 1024, // 1.5MB
            cpu_intensity: 0.8,
            scalability_factor: 0.75,
        }
    }
}

impl LoadAwareStrategy {
    async fn reconstruct_astar_path(
        &self,
        source: &NodeId,
        destination: &NodeId,
        came_from: &HashMap<NodeId, NodeId>,
        topology: &NetworkTopology,
    ) -> Result<Route, RoutingError> {
        let mut path = Vec::new();
        let mut current = *destination;

        while current != *source {
            path.push(current);
            current = *came_from
                .get(&current)
                .ok_or_else(|| RoutingError::NoRouteFound {
                    source: *source,
                    destination: *destination,
                })?;
        }
        path.push(*source);
        path.reverse();

        // Build route similar to Dijkstra
        let mut hops = Vec::new();
        let mut total_cost = 0.0;
        let mut total_latency = Duration::ZERO;
        let mut min_bandwidth = u64::MAX;
        let mut reliability_product = 1.0;

        for window in path.windows(2) {
            let from = window[0];
            let to = window[1];

            let link = topology
                .links
                .values()
                .find(|link| link.from == from && link.to == to)
                .ok_or_else(|| RoutingError::Internal {
                    message: format!("Link not found between {} and {}", from, to),
                })?;

            // Get current load and adjust metrics
            let current_load = self
                .load_monitor
                .get_link_load(&link.id)
                .await
                .unwrap_or(0.0);
            let load_adjusted_cost = self
                .cost_calculator
                .calculate_cost(link, &RoutingContext::default(), Some(current_load))
                .await;

            let hop = Hop {
                from,
                to,
                transport: link.transport,
                cost: load_adjusted_cost,
                latency: link.latency,
                bandwidth: link.bandwidth,
                reliability: link.reliability * (1.0 - current_load * 0.1), // Reduce reliability with load
                security_level: topology
                    .nodes
                    .get(&to)
                    .map(|n| n.security_level)
                    .unwrap_or(SecurityLevel::Internal),
            };

            total_cost += hop.cost;
            total_latency += hop.latency;
            min_bandwidth = min_bandwidth.min(hop.bandwidth);
            reliability_product *= hop.reliability;

            hops.push(hop);
        }

        Ok(Route {
            id: Uuid::new_v4(),
            path,
            hops,
            total_cost,
            estimated_latency: total_latency,
            reliability_score: reliability_product,
            bandwidth_available: min_bandwidth,
            created_at: SystemTime::now(),
            expires_at: Some(SystemTime::now() + Duration::from_secs(180)), // Shorter TTL due to load changes
            metadata: RouteMetadata {
                algorithm_used: "LoadAware".to_string(),
                calculation_time: Instant::now().duration_since(Instant::now()),
                cache_hit: false,
                alternative_routes: 0,
                confidence_score: 0.9,
            },
        })
    }
}

/// Default cost calculator implementation
pub struct DefaultCostCalculator;

#[async_trait::async_trait]
impl CostCalculator for DefaultCostCalculator {
    async fn calculate_cost(
        &self,
        link: &NetworkLink,
        context: &RoutingContext,
        current_load: Option<f64>,
    ) -> f64 {
        let mut cost = link.cost;

        // Adjust for current load
        if let Some(load) = current_load {
            cost *= 1.0 + load * 2.0; // Exponential cost increase with load
        }

        // Adjust for QoS requirements
        match context.qos_requirements.priority {
            MessagePriority::Critical => cost *= 0.5, // Prefer faster routes
            MessagePriority::High => cost *= 0.7,
            MessagePriority::Normal => cost *= 1.0,
            MessagePriority::Low => cost *= 1.2,
            MessagePriority::Background => cost *= 1.5,
        }

        // Adjust for latency requirements
        if let Some(max_latency) = context.qos_requirements.max_latency {
            if link.latency > max_latency {
                cost *= 10.0; // Heavy penalty for exceeding latency requirements
            }
        }

        // Adjust for bandwidth requirements
        if let Some(min_bandwidth) = context.qos_requirements.min_bandwidth {
            if link.bandwidth < min_bandwidth {
                cost *= 5.0; // Penalty for insufficient bandwidth
            }
        }

        // Adjust for reliability requirements
        if let Some(min_reliability) = context.qos_requirements.reliability_threshold {
            if link.reliability < min_reliability {
                cost *= 3.0; // Penalty for insufficient reliability
            }
        }

        cost
    }
}

/// Default heuristic calculator using geographic distance
pub struct DefaultHeuristicCalculator;

impl HeuristicCalculator for DefaultHeuristicCalculator {
    fn calculate_heuristic(
        &self,
        from: &NodeId,
        to: &NodeId,
        topology: &NetworkTopology,
        _context: &RoutingContext,
    ) -> f64 {
        // Simple heuristic: use minimum possible cost based on direct distance
        let from_node = topology.nodes.get(from);
        let to_node = topology.nodes.get(to);

        match (from_node, to_node) {
            (Some(from_node), Some(to_node)) => {
                // If nodes are in the same region, use low cost
                if from_node.region == to_node.region {
                    1.0
                } else {
                    // Different regions, higher cost
                    5.0
                }
            }
            _ => 0.0, // Unknown nodes, no heuristic
        }
    }
}

/// Simple load monitor implementation
pub struct SimpleLoadMonitor {
    link_loads: Arc<RwLock<HashMap<LinkId, f64>>>,
    node_loads: Arc<RwLock<HashMap<NodeId, f64>>>,
}

impl SimpleLoadMonitor {
    pub fn new() -> Self {
        Self {
            link_loads: Arc::new(RwLock::new(HashMap::new())),
            node_loads: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update_link_load(&self, link_id: LinkId, load: f64) {
        let mut loads = self.link_loads.write().await;
        loads.insert(link_id, load);
    }

    pub async fn update_node_load(&self, node_id: NodeId, load: f64) {
        let mut loads = self.node_loads.write().await;
        loads.insert(node_id, load);
    }
}

#[async_trait::async_trait]
impl LoadMonitor for SimpleLoadMonitor {
    async fn get_link_load(&self, link_id: &LinkId) -> Option<f64> {
        let loads = self.link_loads.read().await;
        loads.get(link_id).copied()
    }

    async fn get_node_load(&self, node_id: &NodeId) -> Option<f64> {
        let loads = self.node_loads.read().await;
        loads.get(node_id).copied()
    }

    async fn predict_load(&self, link_id: &LinkId, _duration: Duration) -> Option<f64> {
        // Simple prediction: return current load
        self.get_link_load(link_id).await
    }
}

impl Default for RoutingContext {
    fn default() -> Self {
        Self::new(Uuid::new_v4(), Uuid::new_v4(), QoSRequirements::default())
    }
}
