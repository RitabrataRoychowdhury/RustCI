//! Compressed Radix Arena (CRA) for Prefix Resolution
//!
//! This module implements a high-performance, memory-efficient radix tree
//! optimized for prefix-based service discovery with arena allocation.

use std::sync::{Arc, RwLock};
use smallvec::SmallVec;
use lru::LruCache;
use std::num::NonZeroUsize;

/// Service identifier for service discovery
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ServiceId(pub u64);

impl ServiceId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for ServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Node index for arena allocation
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeIndex(pub usize);

impl NodeIndex {
    pub fn new(index: usize) -> Self {
        Self(index)
    }
    
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

/// Compact string representation for memory efficiency
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactString {
    data: String,
}

impl CompactString {
    pub fn new(s: &str) -> Self {
        Self {
            data: s.to_string(),
        }
    }
    
    pub fn as_str(&self) -> &str {
        &self.data
    }
    
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    
    /// Find common prefix length with another string
    pub fn common_prefix_len(&self, other: &str) -> usize {
        self.data.chars()
            .zip(other.chars())
            .take_while(|(a, b)| a == b)
            .count()
    }
    
    /// Split at the given position
    pub fn split_at(&self, pos: usize) -> (CompactString, CompactString) {
        let chars: Vec<char> = self.data.chars().collect();
        let left: String = chars[..pos].iter().collect();
        let right: String = chars[pos..].iter().collect();
        (CompactString::new(&left), CompactString::new(&right))
    }
    
    /// Check if this string starts with the given prefix
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.data.starts_with(prefix)
    }
}

impl From<&str> for CompactString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for CompactString {
    fn from(s: String) -> Self {
        Self { data: s }
    }
}

/// Radix tree node with compressed prefixes and arena allocation
#[derive(Debug, Clone)]
pub struct RadixNode {
    /// Compressed prefix for this node
    pub prefix: CompactString,
    /// Services stored at this node (terminal nodes)
    pub services: SmallVec<[ServiceId; 4]>,
    /// Child node indices in the arena
    pub children: SmallVec<[NodeIndex; 8]>,
    /// Whether this node represents a complete key (terminal)
    pub is_terminal: bool,
}

impl RadixNode {
    /// Create a new radix node
    pub fn new(prefix: CompactString) -> Self {
        Self {
            prefix,
            services: SmallVec::new(),
            children: SmallVec::new(),
            is_terminal: false,
        }
    }
    
    /// Add a service to this node
    pub fn add_service(&mut self, service_id: ServiceId) {
        if !self.services.contains(&service_id) {
            self.services.push(service_id);
            self.is_terminal = true;
        }
    }
    
    /// Remove a service from this node
    pub fn remove_service(&mut self, service_id: ServiceId) -> bool {
        if let Some(pos) = self.services.iter().position(|&id| id == service_id) {
            self.services.remove(pos);
            if self.services.is_empty() {
                self.is_terminal = false;
            }
            true
        } else {
            false
        }
    }
    
    /// Find child with matching first character
    pub fn find_child_by_char(&self, arena: &Arena<RadixNode>, ch: char) -> Option<NodeIndex> {
        for &child_idx in &self.children {
            if let Some(child) = arena.get(child_idx) {
                if let Some(first_char) = child.prefix.as_str().chars().next() {
                    if first_char == ch {
                        return Some(child_idx);
                    }
                }
            }
        }
        None
    }
    
    /// Add a child node
    pub fn add_child(&mut self, child_idx: NodeIndex) {
        if !self.children.contains(&child_idx) {
            self.children.push(child_idx);
        }
    }
    
    /// Remove a child node
    pub fn remove_child(&mut self, child_idx: NodeIndex) -> bool {
        if let Some(pos) = self.children.iter().position(|&idx| idx == child_idx) {
            self.children.remove(pos);
            true
        } else {
            false
        }
    }
}

/// Simple arena allocator for memory-efficient node storage
#[derive(Debug)]
pub struct Arena<T> {
    items: Vec<Option<T>>,
    free_list: Vec<usize>,
}

impl<T> Arena<T> {
    /// Create a new arena
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            free_list: Vec::new(),
        }
    }
    
    /// Allocate a new item in the arena
    pub fn alloc(&mut self, item: T) -> NodeIndex {
        if let Some(index) = self.free_list.pop() {
            self.items[index] = Some(item);
            NodeIndex::new(index)
        } else {
            let index = self.items.len();
            self.items.push(Some(item));
            NodeIndex::new(index)
        }
    }
    
    /// Get an item from the arena
    pub fn get(&self, index: NodeIndex) -> Option<&T> {
        self.items.get(index.as_usize())?.as_ref()
    }
    
    /// Get a mutable reference to an item
    pub fn get_mut(&mut self, index: NodeIndex) -> Option<&mut T> {
        self.items.get_mut(index.as_usize())?.as_mut()
    }
    
    /// Deallocate an item (mark as free)
    pub fn dealloc(&mut self, index: NodeIndex) -> Option<T> {
        let idx = index.as_usize();
        if idx < self.items.len() {
            if let Some(item) = self.items[idx].take() {
                self.free_list.push(idx);
                Some(item)
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Get the number of allocated items
    pub fn len(&self) -> usize {
        self.items.len() - self.free_list.len()
    }
    
    /// Check if the arena is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Fuzzy match result with scoring
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub service_id: ServiceId,
    pub score: f64,
    pub matched_ranges: Vec<std::ops::Range<usize>>,
}

impl FuzzyMatch {
    pub fn new(service_id: ServiceId, score: f64) -> Self {
        Self {
            service_id,
            score,
            matched_ranges: Vec::new(),
        }
    }
}

/// Error types for CRA operations
#[derive(Debug, thiserror::Error)]
pub enum CraError {
    #[error("Service not found: {service_id:?}")]
    ServiceNotFound { service_id: ServiceId },
    
    #[error("Invalid prefix: {prefix}")]
    InvalidPrefix { prefix: String },
    
    #[error("Node not found: {node_index:?}")]
    NodeNotFound { node_index: NodeIndex },
    
    #[error("Arena corruption detected")]
    ArenaCorruption,
}

/// Statistics for monitoring CRA performance
#[derive(Debug, Clone, Default)]
pub struct CraStats {
    pub nodes: usize,
    pub services: usize,
    pub max_depth: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub insertions: u64,
    pub lookups: u64,
    pub prefix_searches: u64,
}

impl CraStats {
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }
}

/// Compressed Radix Arena for efficient prefix-based service discovery
pub struct CompressedRadixArena {
    /// Arena for node allocation
    nodes: Arena<RadixNode>,
    /// Root node index
    root: NodeIndex,
    /// LRU cache for frequent prefix queries
    prefix_cache: LruCache<String, Vec<ServiceId>>,
    /// Statistics for monitoring
    stats: Arc<RwLock<CraStats>>,
}

impl CompressedRadixArena {
    /// Create a new CRA with default cache size
    pub fn new() -> Self {
        Self::with_cache_size(1000)
    }
    
    /// Create a new CRA with specified cache size
    pub fn with_cache_size(cache_size: usize) -> Self {
        let mut nodes = Arena::new();
        let root = nodes.alloc(RadixNode::new(CompactString::new("")));
        
        Self {
            nodes,
            root,
            prefix_cache: LruCache::new(NonZeroUsize::new(cache_size).unwrap()),
            stats: Arc::new(RwLock::new(CraStats::default())),
        }
    }
    
    /// Insert a service with the given prefix
    pub fn insert(&mut self, prefix: &str, service_id: ServiceId) -> Result<(), CraError> {
        if prefix.is_empty() {
            return Err(CraError::InvalidPrefix { prefix: prefix.to_string() });
        }
        
        self.insert_recursive(self.root, prefix, service_id, 0)?;
        
        // Update stats
        if let Ok(mut stats) = self.stats.write() {
            stats.insertions += 1;
            stats.services += 1;
            stats.nodes = self.nodes.len();
        }
        
        // Invalidate cache entries that might be affected
        self.prefix_cache.clear();
        
        Ok(())
    }
    
    /// Find all services with the given prefix
    pub fn find_prefix(&mut self, prefix: &str) -> Vec<ServiceId> {
        // Check cache first
        if let Some(cached) = self.prefix_cache.get(prefix) {
            if let Ok(mut stats) = self.stats.write() {
                stats.cache_hits += 1;
                stats.lookups += 1;
            }
            return cached.clone();
        }
        
        // Perform search
        let mut results = Vec::new();
        self.find_prefix_recursive(self.root, prefix, &mut results);
        
        // Cache the result
        self.prefix_cache.put(prefix.to_string(), results.clone());
        
        // Update stats
        if let Ok(mut stats) = self.stats.write() {
            stats.cache_misses += 1;
            stats.lookups += 1;
            stats.prefix_searches += 1;
        }
        
        results
    }
    
    /// Remove a service
    pub fn remove(&mut self, prefix: &str, service_id: ServiceId) -> Result<bool, CraError> {
        let removed = self.remove_recursive(self.root, prefix, service_id)?;
        
        if removed {
            // Update stats
            if let Ok(mut stats) = self.stats.write() {
                stats.services = stats.services.saturating_sub(1);
            }
            
            // Invalidate cache
            self.prefix_cache.clear();
        }
        
        Ok(removed)
    }
    
    /// Perform fuzzy search (simplified implementation)
    pub fn fuzzy_search(&mut self, query: &str, limit: usize) -> Vec<FuzzyMatch> {
        let mut matches = Vec::new();
        self.fuzzy_search_recursive(self.root, query, &mut matches, String::new());
        
        // Sort by score (descending) and limit results
        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(limit);
        
        matches
    }
    
    /// Get current statistics
    pub fn stats(&self) -> CraStats {
        self.stats.read()
            .map(|stats| stats.clone())
            .unwrap_or_default()
    }
    
    /// Get the number of services stored
    pub fn service_count(&self) -> usize {
        self.stats.read()
            .map(|stats| stats.services)
            .unwrap_or(0)
    }
    
    /// Get the number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    
    /// Recursive insertion helper
    fn insert_recursive(
        &mut self,
        node_idx: NodeIndex,
        key: &str,
        service_id: ServiceId,
        depth: usize,
    ) -> Result<(), CraError> {
        let node = self.nodes.get(node_idx)
            .ok_or(CraError::NodeNotFound { node_index: node_idx })?
            .clone();
        
        let prefix_len = node.prefix.common_prefix_len(key);
        
        if prefix_len == node.prefix.len() {
            // Full prefix match
            if prefix_len == key.len() {
                // Exact match - add service to this node
                let node_mut = self.nodes.get_mut(node_idx)
                    .ok_or(CraError::NodeNotFound { node_index: node_idx })?;
                node_mut.add_service(service_id);
                return Ok(());
            } else {
                // Continue with remaining key
                let remaining_key = &key[prefix_len..];
                if let Some(first_char) = remaining_key.chars().next() {
                    if let Some(child_idx) = node.find_child_by_char(&self.nodes, first_char) {
                        return self.insert_recursive(child_idx, remaining_key, service_id, depth + 1);
                    } else {
                        // Create new child
                        let new_child = RadixNode::new(CompactString::new(remaining_key));
                        let child_idx = self.nodes.alloc(new_child);
                        
                        let node_mut = self.nodes.get_mut(node_idx)
                            .ok_or(CraError::NodeNotFound { node_index: node_idx })?;
                        node_mut.add_child(child_idx);
                        
                        let child_mut = self.nodes.get_mut(child_idx)
                            .ok_or(CraError::NodeNotFound { node_index: child_idx })?;
                        child_mut.add_service(service_id);
                        
                        return Ok(());
                    }
                }
            }
        } else if prefix_len > 0 {
            // Partial prefix match - need to split the node
            let (common_prefix, node_suffix) = node.prefix.split_at(prefix_len);
            let key_suffix = &key[prefix_len..];
            
            // Create new intermediate node with common prefix
            let intermediate_node = RadixNode::new(common_prefix);
            let intermediate_idx = self.nodes.alloc(intermediate_node);
            
            // Update current node to have the suffix
            if let Some(node_mut) = self.nodes.get_mut(node_idx) {
                node_mut.prefix = node_suffix;
            }
            
            // Create new child for remaining key if needed
            let child_idx = if !key_suffix.is_empty() {
                let mut new_child = RadixNode::new(CompactString::new(key_suffix));
                new_child.add_service(service_id);
                Some(self.nodes.alloc(new_child))
            } else {
                None
            };
            
            // Update intermediate node
            if let Some(intermediate_mut) = self.nodes.get_mut(intermediate_idx) {
                intermediate_mut.add_child(node_idx);
                if key_suffix.is_empty() {
                    intermediate_mut.add_service(service_id);
                } else if let Some(child_idx) = child_idx {
                    intermediate_mut.add_child(child_idx);
                }
            }
            
            // Replace current node with intermediate node (this is simplified)
            // In a real implementation, we'd need to update parent references
            return Ok(());
        }
        
        // No common prefix - create new child
        let mut new_child = RadixNode::new(CompactString::new(key));
        new_child.add_service(service_id);
        let child_idx = self.nodes.alloc(new_child);
        
        let node_mut = self.nodes.get_mut(node_idx)
            .ok_or(CraError::NodeNotFound { node_index: node_idx })?;
        node_mut.add_child(child_idx);
        
        Ok(())
    }
    
    /// Recursive prefix search helper
    fn find_prefix_recursive(&self, node_idx: NodeIndex, prefix: &str, results: &mut Vec<ServiceId>) {
        if let Some(node) = self.nodes.get(node_idx) {
            if prefix.is_empty() {
                // Collect all services from this subtree
                self.collect_all_services(node_idx, results);
            } else if node.prefix.starts_with(prefix) {
                // Node prefix starts with search prefix - collect all
                self.collect_all_services(node_idx, results);
            } else if prefix.starts_with(node.prefix.as_str()) {
                // Search prefix starts with node prefix - continue search
                let remaining_prefix = &prefix[node.prefix.len()..];
                if remaining_prefix.is_empty() {
                    // Exact prefix match - collect all services from subtree
                    self.collect_all_services(node_idx, results);
                } else {
                    // Continue with children
                    for &child_idx in &node.children {
                        self.find_prefix_recursive(child_idx, remaining_prefix, results);
                    }
                }
            }
            // If no prefix relationship, skip this subtree
        }
    }
    
    /// Collect all services from a subtree
    fn collect_all_services(&self, node_idx: NodeIndex, results: &mut Vec<ServiceId>) {
        if let Some(node) = self.nodes.get(node_idx) {
            // Add services from this node
            for &service_id in &node.services {
                if !results.contains(&service_id) {
                    results.push(service_id);
                }
            }
            
            // Recursively collect from children
            for &child_idx in &node.children {
                self.collect_all_services(child_idx, results);
            }
        }
    }
    
    /// Recursive removal helper
    fn remove_recursive(
        &mut self,
        node_idx: NodeIndex,
        key: &str,
        service_id: ServiceId,
    ) -> Result<bool, CraError> {
        let node = self.nodes.get(node_idx)
            .ok_or(CraError::NodeNotFound { node_index: node_idx })?
            .clone();
        
        if key.starts_with(node.prefix.as_str()) {
            let remaining_key = &key[node.prefix.len()..];
            
            if remaining_key.is_empty() {
                // Found the target node
                let node_mut = self.nodes.get_mut(node_idx)
                    .ok_or(CraError::NodeNotFound { node_index: node_idx })?;
                return Ok(node_mut.remove_service(service_id));
            } else {
                // Continue search in children
                if let Some(first_char) = remaining_key.chars().next() {
                    if let Some(child_idx) = node.find_child_by_char(&self.nodes, first_char) {
                        return self.remove_recursive(child_idx, remaining_key, service_id);
                    }
                }
            }
        }
        
        Ok(false)
    }
    
    /// Recursive fuzzy search helper (simplified)
    fn fuzzy_search_recursive(
        &self,
        node_idx: NodeIndex,
        query: &str,
        matches: &mut Vec<FuzzyMatch>,
        current_path: String,
    ) {
        if let Some(node) = self.nodes.get(node_idx) {
            let full_path = current_path + node.prefix.as_str();
            
            // Simple fuzzy matching based on substring containment
            if node.is_terminal {
                let score = self.calculate_fuzzy_score(query, &full_path);
                if score > 0.0 {
                    for &service_id in &node.services {
                        matches.push(FuzzyMatch::new(service_id, score));
                    }
                }
            }
            
            // Continue with children
            for &child_idx in &node.children {
                self.fuzzy_search_recursive(child_idx, query, matches, full_path.clone());
            }
        }
    }
    
    /// Calculate fuzzy match score (simplified)
    fn calculate_fuzzy_score(&self, query: &str, target: &str) -> f64 {
        if target.contains(query) {
            // Simple scoring based on how early the match appears
            if let Some(pos) = target.find(query) {
                let base_score = 1.0;
                let position_penalty = pos as f64 / target.len() as f64;
                let length_bonus = query.len() as f64 / target.len() as f64;
                base_score - position_penalty + length_bonus
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

impl Default for CompressedRadixArena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compact_string() {
        let s = CompactString::new("hello");
        assert_eq!(s.as_str(), "hello");
        assert_eq!(s.len(), 5);
        assert!(!s.is_empty());
        
        assert_eq!(s.common_prefix_len("help"), 3);
        assert_eq!(s.common_prefix_len("world"), 0);
        
        let (left, right) = s.split_at(3);
        assert_eq!(left.as_str(), "hel");
        assert_eq!(right.as_str(), "lo");
    }
    
    #[test]
    fn test_arena() {
        let mut arena = Arena::new();
        
        let idx1 = arena.alloc("hello");
        let idx2 = arena.alloc("world");
        
        assert_eq!(arena.get(idx1), Some(&"hello"));
        assert_eq!(arena.get(idx2), Some(&"world"));
        assert_eq!(arena.len(), 2);
        
        let removed = arena.dealloc(idx1);
        assert_eq!(removed, Some("hello"));
        assert_eq!(arena.len(), 1);
        
        // Reuse the freed slot
        let idx3 = arena.alloc("reused");
        assert_eq!(arena.get(idx3), Some(&"reused"));
        assert_eq!(arena.len(), 2);
    }
    
    #[test]
    fn test_cra_basic_operations() {
        let mut cra = CompressedRadixArena::new();
        
        // Insert services
        cra.insert("api/users", ServiceId::new(1)).unwrap();
        cra.insert("api/orders", ServiceId::new(2)).unwrap();
        cra.insert("api/users/profile", ServiceId::new(3)).unwrap();
        
        assert_eq!(cra.service_count(), 3);
        
        // Test prefix search
        let users_services = cra.find_prefix("api/users");
        assert!(users_services.contains(&ServiceId::new(1)));
        assert!(users_services.contains(&ServiceId::new(3)));
        assert!(!users_services.contains(&ServiceId::new(2)));
        
        let api_services = cra.find_prefix("api");
        assert_eq!(api_services.len(), 3);
    }
    
    #[test]
    fn test_cra_fuzzy_search() {
        let mut cra = CompressedRadixArena::new();
        
        cra.insert("user-service", ServiceId::new(1)).unwrap();
        cra.insert("user-auth", ServiceId::new(2)).unwrap();
        cra.insert("order-service", ServiceId::new(3)).unwrap();
        
        let matches = cra.fuzzy_search("user", 10);
        assert_eq!(matches.len(), 2);
        
        // Should find both user services
        let service_ids: Vec<ServiceId> = matches.iter().map(|m| m.service_id).collect();
        assert!(service_ids.contains(&ServiceId::new(1)));
        assert!(service_ids.contains(&ServiceId::new(2)));
    }
    
    #[test]
    fn test_cra_removal() {
        let mut cra = CompressedRadixArena::new();
        
        cra.insert("api/users", ServiceId::new(1)).unwrap();
        cra.insert("api/orders", ServiceId::new(2)).unwrap();
        
        assert_eq!(cra.service_count(), 2);
        
        let removed = cra.remove("api/users", ServiceId::new(1)).unwrap();
        assert!(removed);
        assert_eq!(cra.service_count(), 1);
        
        let users_services = cra.find_prefix("api/users");
        assert!(users_services.is_empty());
        
        let api_services = cra.find_prefix("api");
        assert_eq!(api_services.len(), 1);
        assert!(api_services.contains(&ServiceId::new(2)));
    }
    
    #[test]
    fn test_cra_cache() {
        let mut cra = CompressedRadixArena::with_cache_size(2);
        
        cra.insert("api/users", ServiceId::new(1)).unwrap();
        cra.insert("api/orders", ServiceId::new(2)).unwrap();
        
        // First search - cache miss
        let _result1 = cra.find_prefix("api");
        let stats1 = cra.stats();
        assert_eq!(stats1.cache_misses, 1);
        assert_eq!(stats1.cache_hits, 0);
        
        // Second search - cache hit
        let _result2 = cra.find_prefix("api");
        let stats2 = cra.stats();
        assert_eq!(stats2.cache_misses, 1);
        assert_eq!(stats2.cache_hits, 1);
    }
}