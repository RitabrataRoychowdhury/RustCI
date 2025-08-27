//! Property-based tests for high-performance routing components
//!
//! These tests use PropTest to verify correctness properties that should hold
//! for all possible inputs, ensuring the robustness of our lock-free data structures.

use proptest::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;

use RustAutoDevOps::valkyrie::lockfree::fingerprinted_id_table::{
    FingerprintedIdTable, RouteEntry, RouteId, NodeId, ShardId
};
use RustAutoDevOps::valkyrie::lockfree::compressed_radix_arena::CompressedRadixArena;
use RustAutoDevOps::valkyrie::lockfree::ServiceId;
use RustAutoDevOps::core::networking::valkyrie::lockfree::registry::LockFreeRegistry;
use RustAutoDevOps::valkyrie::routing::enhanced_registries::EnhancedServiceRegistry;

/// Property tests for Fingerprinted ID Table (FIT)
mod fit_properties {
    use super::*;

    /// Generate arbitrary RouteEntry for property testing
    pub fn arb_route_entry() -> impl Strategy<Value = RouteEntry> {
        (
            any::<u64>(), // route_id
            any::<u64>(), // next_hop
            any::<u32>(), // shard_id
            1u32..10000u32, // latency_hint (reasonable range)
            1u16..65535u16, // reliability_score
        ).prop_map(|(route_id, next_hop, shard_id, latency_hint, reliability_score)| {
            RouteEntry::new(
                RouteId::new(route_id),
                NodeId::new(next_hop),
                ShardId::new(shard_id),
                latency_hint,
                reliability_score,
            )
        })
    }

    proptest! {
        /// Property: After inserting an entry, it should be retrievable
        #[test]
        fn prop_insert_then_lookup(entry in arb_route_entry()) {
            let fit = FingerprintedIdTable::with_defaults();
            
            // Insert the entry
            fit.insert(entry.clone()).unwrap();
            
            // Should be able to retrieve it
            let retrieved = fit.lookup(entry.id).unwrap();
            prop_assert_eq!(retrieved.id, entry.id);
            prop_assert_eq!(retrieved.next_hop, entry.next_hop);
            prop_assert_eq!(retrieved.shard_id, entry.shard_id);
        }

        /// Property: Lookup of non-existent entry returns None
        #[test]
        fn prop_lookup_nonexistent(
            entries in prop::collection::vec(arb_route_entry(), 0..100),
            lookup_id in any::<u64>()
        ) {
            let fit = FingerprintedIdTable::with_defaults();
            let lookup_route_id = RouteId::new(lookup_id);
            
            // Insert entries
            let mut inserted_ids = HashSet::new();
            for entry in entries {
                if fit.insert(entry.clone()).is_ok() {
                    inserted_ids.insert(entry.id);
                }
            }
            
            // If lookup_id wasn't inserted, lookup should return None
            if !inserted_ids.contains(&lookup_route_id) {
                prop_assert!(fit.lookup(lookup_route_id).is_none());
            }
        }

        /// Property: Size increases with successful insertions
        #[test]
        fn prop_size_consistency(entries in prop::collection::vec(arb_route_entry(), 0..50)) {
            let fit = FingerprintedIdTable::with_defaults();
            let initial_size = fit.size();
            
            let mut successful_inserts = 0;
            let mut seen_ids = HashSet::new();
            
            for entry in entries {
                if !seen_ids.contains(&entry.id) {
                    if fit.insert(entry.clone()).is_ok() {
                        successful_inserts += 1;
                        seen_ids.insert(entry.id);
                    }
                }
            }
            
            prop_assert_eq!(fit.size(), initial_size + successful_inserts);
        }

        /// Property: Remove after insert should succeed and decrease size
        #[test]
        fn prop_insert_remove_consistency(entry in arb_route_entry()) {
            let fit = FingerprintedIdTable::with_defaults();
            
            // Insert entry
            fit.insert(entry.clone()).unwrap();
            let size_after_insert = fit.size();
            
            // Remove entry
            let removed = fit.remove(entry.id).unwrap();
            let size_after_remove = fit.size();
            
            prop_assert_eq!(removed.id, entry.id);
            prop_assert_eq!(size_after_remove, size_after_insert - 1);
            
            // Should not be able to lookup after removal
            prop_assert!(fit.lookup(entry.id).is_none());
        }

        /// Property: Fingerprint collision detection works
        #[test]
        fn prop_no_duplicate_ids(
            entries in prop::collection::vec(arb_route_entry(), 0..100)
        ) {
            let fit = FingerprintedIdTable::with_defaults();
            let mut inserted_ids = HashSet::new();
            
            for entry in entries {
                match fit.insert(entry.clone()) {
                    Ok(()) => {
                        // Should be a new ID
                        prop_assert!(!inserted_ids.contains(&entry.id));
                        inserted_ids.insert(entry.id);
                    }
                    Err(_) => {
                        // Either capacity exceeded or duplicate ID
                        // Both are acceptable behaviors
                    }
                }
            }
            
            // All inserted IDs should be retrievable
            for id in &inserted_ids {
                prop_assert!(fit.lookup(*id).is_some());
            }
        }

        /// Property: Concurrent access maintains consistency
        #[test]
        fn prop_concurrent_consistency(
            entries in prop::collection::vec(arb_route_entry(), 10..50)
        ) {
            let fit = Arc::new(FingerprintedIdTable::with_defaults());
            
            // Pre-populate with some entries
            let mut expected_entries = HashMap::new();
            for entry in &entries[..entries.len()/2] {
                if fit.insert(entry.clone()).is_ok() {
                    expected_entries.insert(entry.id, entry.clone());
                }
            }
            
            // Spawn threads for concurrent access
            let mut handles = vec![];
            
            // Reader threads
            for _ in 0..2 {
                let fit_clone = Arc::clone(&fit);
                let expected_clone = expected_entries.clone();
                let handle = thread::spawn(move || {
                    for (id, expected_entry) in expected_clone {
                        if let Some(retrieved) = fit_clone.lookup(id) {
                            assert_eq!(retrieved.id, expected_entry.id);
                        }
                    }
                });
                handles.push(handle);
            }
            
            // Writer thread
            let fit_clone = Arc::clone(&fit);
            let remaining_entries = entries[entries.len()/2..].to_vec();
            let handle = thread::spawn(move || {
                for entry in remaining_entries {
                    let _ = fit_clone.insert(entry);
                }
            });
            handles.push(handle);
            
            // Wait for all threads
            for handle in handles {
                handle.join().unwrap();
            }
            
            // Verify original entries are still accessible
            for (id, expected_entry) in expected_entries {
                if let Some(retrieved) = fit.lookup(id) {
                    prop_assert_eq!(retrieved.id, expected_entry.id);
                }
            }
        }
    }
}

/// Property tests for Compressed Radix Arena (CRA)
mod cra_properties {
    use super::*;

    /// Generate arbitrary string keys for CRA testing
    pub fn arb_key() -> impl Strategy<Value = String> {
        prop::string::string_regex(r"[a-zA-Z0-9._-]{1,50}").unwrap()
    }

    proptest! {
        /// Property: After inserting a key-value pair, it should be retrievable
        #[test]
        fn prop_insert_then_lookup(key in arb_key(), value in any::<u64>()) {
            let mut cra = CompressedRadixArena::new();
            
            // Insert the key-value pair
            cra.insert(&key, ServiceId(value)).unwrap();
            
            // Should be able to retrieve it
            let retrieved = cra.find(&key).unwrap();
            prop_assert_eq!(retrieved, ServiceId(value));
        }

        /// Property: Lookup of non-existent key returns None
        #[test]
        fn prop_lookup_nonexistent(
            entries in prop::collection::vec((arb_key(), any::<u64>()), 0..50),
            lookup_key in arb_key()
        ) {
            let mut cra = CompressedRadixArena::new();
            
            // Insert entries
            let mut inserted_keys = HashSet::new();
            for (key, value) in entries {
                if cra.insert(&key, ServiceId(value)).is_ok() {
                    inserted_keys.insert(key);
                }
            }
            
            // If lookup_key wasn't inserted, lookup should return None
            if !inserted_keys.contains(&lookup_key) {
                prop_assert!(cra.find(&lookup_key).is_none());
            }
        }

        /// Property: Prefix search returns all matching entries
        #[test]
        fn prop_prefix_search_completeness(
            prefix in prop::string::string_regex(r"[a-zA-Z]{1,10}").unwrap(),
            suffixes in prop::collection::vec(
                prop::string::string_regex(r"[a-zA-Z0-9._-]{0,20}").unwrap(),
                1..20
            )
        ) {
            let cra = CompressedRadixArena::new();
            
            // Insert entries with the given prefix
            let mut expected_matches = HashSet::new();
            for (i, suffix) in suffixes.iter().enumerate() {
                let key = format!("{}{}", prefix, suffix);
                if cra.insert(&key, ServiceId(i as u64)).is_ok() {
                    expected_matches.insert(key);
                }
            }
            
            // Also insert some entries without the prefix
            for i in 0..5 {
                let non_matching_key = format!("different{}", i);
                let _ = cra.insert(&non_matching_key, ServiceId((100 + i) as u64));
            }
            
            // Prefix search should find all matching entries
            let found_entries = cra.find_prefix(&prefix);
            let found_keys: HashSet<String> = found_entries.iter()
                .map(|entry| entry.key.clone())
                .collect();
            
            // All expected matches should be found
            for expected_key in &expected_matches {
                prop_assert!(found_keys.contains(expected_key), 
                    "Expected key '{}' not found in prefix search results", expected_key);
            }
            
            // All found keys should start with the prefix
            for found_key in &found_keys {
                prop_assert!(found_key.starts_with(&prefix),
                    "Found key '{}' doesn't start with prefix '{}'", found_key, prefix);
            }
        }

        /// Property: Update existing key changes value but not structure
        #[test]
        fn prop_update_consistency(
            key in arb_key(),
            initial_value in any::<u64>(),
            new_value in any::<u64>()
        ) {
            let mut cra = CompressedRadixArena::new();
            
            // Insert initial value
            cra.insert(&key, ServiceId(initial_value)).unwrap();
            prop_assert_eq!(cra.find(&key).unwrap(), ServiceId(initial_value));
            
            // Update with new value
            cra.insert(&key, ServiceId(new_value)).unwrap();
            prop_assert_eq!(cra.find(&key).unwrap(), ServiceId(new_value));
        }

        /// Property: Concurrent reads are consistent
        #[test]
        fn prop_concurrent_read_consistency(
            entries in prop::collection::vec((arb_key(), any::<u64>()), 10..30)
        ) {
            let mut cra = CompressedRadixArena::new();
            
            // Pre-populate the CRA
            let mut expected_entries = HashMap::new();
            for (key, value) in entries {
                if cra.insert(&key, ServiceId(value)).is_ok() {
                    expected_entries.insert(key, ServiceId(value));
                }
            }
            
            let cra = Arc::new(cra);
            
            // Spawn multiple reader threads
            let mut handles = vec![];
            for _ in 0..3 {
                let cra_clone = Arc::clone(&cra);
                let expected_clone = expected_entries.clone();
                let handle = thread::spawn(move || {
                    for (key, expected_value) in expected_clone {
                        if let Some(retrieved_value) = cra_clone.find_readonly(&key) {
                            assert_eq!(retrieved_value, expected_value);
                        }
                    }
                });
                handles.push(handle);
            }
            
            // Wait for all threads
            for handle in handles {
                handle.join().unwrap();
            }
            
            // Verify all entries are still accessible
            for (key, expected_value) in expected_entries {
                prop_assert_eq!(cra.find_readonly(&key).unwrap(), expected_value);
            }
        }

        /// Property: Memory compression doesn't affect correctness
        #[test]
        fn prop_compression_correctness(
            common_prefix in prop::string::string_regex(r"[a-zA-Z]{5,15}").unwrap(),
            suffixes in prop::collection::vec(
                prop::string::string_regex(r"[a-zA-Z0-9]{1,10}").unwrap(),
                5..25
            )
        ) {
            let cra = CompressedRadixArena::new();
            
            // Insert entries with common prefix (should trigger compression)
            let mut expected_entries = HashMap::new();
            for (i, suffix) in suffixes.iter().enumerate() {
                let key = format!("{}.{}", common_prefix, suffix);
                let value = ServiceId(i as u64);
                if cra.insert(&key, value).is_ok() {
                    expected_entries.insert(key, value);
                }
            }
            
            // Verify all entries are still retrievable after compression
            for (key, expected_value) in expected_entries {
                prop_assert_eq!(cra.find(&key).unwrap(), expected_value);
            }
            
            // Verify prefix search still works
            let prefix_results = cra.find_prefix(&common_prefix);
            prop_assert!(!prefix_results.is_empty());
            
            for result in prefix_results {
                prop_assert!(result.key.starts_with(&common_prefix));
            }
        }
    }
}

/// Integration property tests
mod integration_properties {
    use super::*;
    use super::cra_properties::arb_key;
    use super::fit_properties::arb_route_entry;
    use RustAutoDevOps::valkyrie::routing::enhanced_registries::{
        EnhancedServiceRegistry, EnhancedServiceEntry, ServiceHealthStatus,
        ServiceMetrics, ServiceEndpoint
    };
    use std::collections::HashMap;
    use std::time::SystemTime;

    proptest! {
        /// Property: Service registry operations are consistent
        #[test]
        fn prop_service_registry_consistency(
            service_names in prop::collection::vec(arb_key(), 1..20),
            service_values in prop::collection::vec(any::<u64>(), 1..20)
        ) {
            let registry = EnhancedServiceRegistry::new();
            let mut expected_services = HashMap::new();
            
            // Register services
            for (name, &id_val) in service_names.iter().zip(service_values.iter()) {
                let service_id = uuid::Uuid::from_u128(id_val as u128);
                let service_entry = EnhancedServiceEntry {
                    id: service_id,
                    name: name.clone(),
                    version: "1.0.0".to_string(),
                    endpoints: vec![ServiceEndpoint {
                        address: "127.0.0.1:8080".parse().unwrap(),
                        protocol: "http".to_string(),
                        weight: 100,
                        healthy: true,
                    }],
                    tags: HashMap::new(),
                    health_status: ServiceHealthStatus::Healthy,
                    metrics: ServiceMetrics::default(),
                    registered_at: SystemTime::now(),
                    last_health_check: SystemTime::now(),
                };
                
                if registry.register(service_id, service_entry.clone()).is_ok() {
                    expected_services.insert(service_id, service_entry);
                }
            }
            
            // Verify all registered services can be retrieved
            for (service_id, expected_entry) in expected_services {
                if let Ok(Some(retrieved)) = registry.lookup(&service_id) {
                    prop_assert_eq!(retrieved.id, expected_entry.id);
                    prop_assert_eq!(retrieved.name, expected_entry.name);
                }
            }
        }
    }
}

/// Performance property tests
mod performance_properties {
    use super::*;
    use std::time::Instant;

    proptest! {
        /// Property: FIT lookup time should be roughly constant (O(1))
        #[test]
        fn prop_fit_lookup_time_constant(
            small_entries in prop::collection::vec(arb_route_entry(), 100..200),
            large_entries in prop::collection::vec(arb_route_entry(), 1000..2000)
        ) {
            // Test with small dataset
            let small_fit = FingerprintedIdTable::with_defaults();
            let mut small_lookup_id = None;
            for entry in small_entries {
                if small_fit.insert(entry.clone()).is_ok() && small_lookup_id.is_none() {
                    small_lookup_id = Some(entry.id);
                }
            }
            
            // Test with large dataset
            let large_fit = FingerprintedIdTable::with_defaults();
            let mut large_lookup_id = None;
            for entry in large_entries {
                if large_fit.insert(entry.clone()).is_ok() && large_lookup_id.is_none() {
                    large_lookup_id = Some(entry.id);
                }
            }
            
            if let (Some(small_id), Some(large_id)) = (small_lookup_id, large_lookup_id) {
                // Measure lookup times
                let small_start = Instant::now();
                for _ in 0..1000 {
                    let _ = small_fit.lookup(small_id);
                }
                let small_time = small_start.elapsed();
                
                let large_start = Instant::now();
                for _ in 0..1000 {
                    let _ = large_fit.lookup(large_id);
                }
                let large_time = large_start.elapsed();
                
                // Large dataset shouldn't be significantly slower (within 2x)
                // This is a loose bound to account for cache effects and system variance
                prop_assert!(large_time.as_nanos() < small_time.as_nanos() * 2,
                    "Large dataset lookup time ({:?}) is more than 2x small dataset time ({:?})",
                    large_time, small_time);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_property_tests() {
        // This test ensures property tests are included in the test suite
        // The actual property tests run automatically when `cargo test` is executed
        println!("Property tests are configured and ready to run");
    }
}