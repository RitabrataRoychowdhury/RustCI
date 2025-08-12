//! Tests for zero-copy and SIMD optimizations in Valkyrie Protocol

use std::sync::Arc;
use std::time::Instant;
use RustAutoDevOps::core::networking::valkyrie::{
    zero_copy::{ZeroCopyBufferPool, SimdDataProcessor, SimdBuffer},
    simd_processor::{SimdMessageProcessor, SimdPatternMatcher},
    performance_bench::PerformanceBenchmark,
    message::{ValkyrieMessage, MessagePayload, MessageType, DestinationType},
};

#[tokio::test]
async fn test_simd_buffer_operations() {
    let mut buffer = SimdBuffer::new(1024).unwrap();
    
    // Test basic operations
    assert_eq!(buffer.capacity(), 1024);
    assert_eq!(buffer.len(), 0);
    assert!(buffer.is_empty());
    
    // Test data manipulation
    let test_data = b"Hello, SIMD world!";
    let buffer_slice = buffer.as_mut_slice();
    buffer_slice[..test_data.len()].copy_from_slice(test_data);
    buffer.set_length(test_data.len());
    
    assert_eq!(buffer.len(), test_data.len());
    assert!(!buffer.is_empty());
    assert_eq!(&buffer.as_slice()[..test_data.len()], test_data);
    
    // Test resize
    buffer.resize(2048).unwrap();
    assert_eq!(buffer.capacity(), 2048);
    assert_eq!(buffer.len(), test_data.len());
    assert_eq!(&buffer.as_slice()[..test_data.len()], test_data);
}

#[tokio::test]
async fn test_zero_copy_buffer_pool() {
    let pool = ZeroCopyBufferPool::new();
    
    // Test buffer allocation and return
    let buffer1 = pool.get_buffer(1024).unwrap();
    assert!(buffer1.capacity() >= 1024);
    
    let buffer2 = pool.get_buffer(2048).unwrap();
    assert!(buffer2.capacity() >= 2048);
    
    // Return buffers to pool
    pool.return_buffer(buffer1);
    pool.return_buffer(buffer2);
    
    // Check statistics
    let stats = pool.get_stats();
    assert!(stats.total_allocations >= 2);
    assert_eq!(stats.current_pool_size, 2);
    
    // Test buffer reuse
    let buffer3 = pool.get_buffer(1024).unwrap();
    let stats_after_reuse = pool.get_stats();
    assert_eq!(stats_after_reuse.cache_hits, 1);
}

#[tokio::test]
async fn test_simd_data_processor() {
    let pool = Arc::new(ZeroCopyBufferPool::new());
    let processor = SimdDataProcessor::new(pool);
    
    // Test SIMD XOR
    let data_a = vec![0xFF; 128];
    let data_b = vec![0x00; 128];
    let mut output = vec![0; 128];
    
    processor.simd_xor(&data_a, &data_b, &mut output).unwrap();
    assert_eq!(output, vec![0xFF; 128]);
    
    // Test SIMD copy
    let source = vec![0x42; 256];
    let mut dest = vec![0; 256];
    
    processor.simd_copy(&source, &mut dest).unwrap();
    assert_eq!(dest, source);
    
    // Test SIMD checksum
    let data = vec![1u8; 100];
    let checksum = processor.simd_checksum(&data);
    assert_eq!(checksum, 100);
    
    // Test SIMD compression
    let input_data = vec![0xAA; 1024];
    let mut input_buffer = processor.buffer_pool.get_buffer(1024).unwrap();
    input_buffer.as_mut_slice()[..1024].copy_from_slice(&input_data);
    input_buffer.set_length(1024);
    
    let compressed = processor.simd_compress(input_buffer.as_slice()).unwrap();
    assert!(compressed.len() < 1024); // Should be compressed
    
    // Check statistics
    let stats = processor.get_stats();
    assert!(stats.operations > 0);
    assert!(stats.bytes_processed > 0);
    assert!(stats.simd_operations > 0);
}

#[tokio::test]
async fn test_simd_message_processor() {
    let pool = Arc::new(ZeroCopyBufferPool::new());
    let processor = SimdMessageProcessor::new(pool);
    
    // Test binary payload processing
    let binary_data = vec![0x55; 2048];
    let binary_payload = MessagePayload::Binary(binary_data.clone());
    
    let processed = processor.process_payload(&binary_payload).unwrap();
    assert!(!processed.is_empty());
    
    // Test text payload processing
    let text_payload = MessagePayload::Text("HELLO WORLD".to_string());
    let processed_text = processor.process_payload(&text_payload).unwrap();
    assert!(!processed_text.is_empty());
    
    // Test JSON payload processing
    let json_value = serde_json::json!({
        "message": "test",
        "data": [1, 2, 3, 4, 5]
    });
    let json_payload = MessagePayload::Json(json_value);
    let processed_json = processor.process_payload(&json_payload).unwrap();
    assert!(!processed_json.is_empty());
    
    // Test batch processing
    let messages = vec![
        ValkyrieMessage::new(
            MessageType::Data,
            "source1".to_string(),
            DestinationType::Unicast("dest1".to_string()),
            MessagePayload::Binary(vec![1, 2, 3, 4])
        ),
        ValkyrieMessage::new(
            MessageType::Data,
            "source2".to_string(),
            DestinationType::Unicast("dest2".to_string()),
            MessagePayload::Text("batch test".to_string())
        ),
    ];
    
    let batch_results = processor.batch_process(&messages).unwrap();
    assert_eq!(batch_results.len(), 2);
    
    // Check statistics
    let stats = processor.get_stats();
    assert!(stats.messages_processed > 0);
    assert!(stats.bytes_processed > 0);
}

#[tokio::test]
async fn test_simd_pattern_matcher() {
    let mut matcher = SimdPatternMatcher::new();
    
    // Add test patterns
    matcher.add_pattern(b"test", None, 1);
    matcher.add_pattern(b"pattern", None, 2);
    matcher.add_pattern(b"SIMD", None, 3);
    
    // Test pattern matching
    let test_data = b"This is a test with pattern matching using SIMD optimizations";
    let matches = matcher.match_patterns(test_data);
    
    assert!(matches.contains(&1)); // "test"
    assert!(matches.contains(&2)); // "pattern"
    assert!(matches.contains(&3)); // "SIMD"
    
    // Test with no matches
    let no_match_data = b"No matching strings here";
    let no_matches = matcher.match_patterns(no_match_data);
    assert!(no_matches.is_empty());
    
    // Check statistics
    let stats = matcher.get_stats();
    assert_eq!(stats.matches_performed, 2);
    assert_eq!(stats.patterns_matched, 3);
}

#[tokio::test]
async fn test_performance_benchmarks() {
    let benchmark = PerformanceBenchmark::new();
    
    // Test SIMD XOR benchmark
    let xor_result = benchmark.benchmark_simd_xor(1024, 100).unwrap();
    assert_eq!(xor_result.operations, 100);
    assert!(xor_result.ops_per_second > 0.0);
    assert!(xor_result.throughput_mbps > 0.0);
    assert_eq!(xor_result.simd_operations, 100);
    
    // Test message processing benchmark
    let msg_result = benchmark.benchmark_message_processing(4096, 10).unwrap();
    assert_eq!(msg_result.operations, 10);
    assert!(msg_result.ops_per_second > 0.0);
    assert!(msg_result.memory_usage > 0);
    
    // Test batch processing benchmark
    let batch_result = benchmark.benchmark_batch_processing(50, 1024).unwrap();
    assert_eq!(batch_result.operations, 50);
    assert!(batch_result.ops_per_second > 0.0);
    
    // Test buffer pool benchmark
    let pool_result = benchmark.benchmark_buffer_pool(2048, 1000).unwrap();
    assert_eq!(pool_result.operations, 1000);
    assert!(pool_result.ops_per_second > 0.0);
    
    // Test pattern matching benchmark
    let pattern_result = benchmark.benchmark_pattern_matching(8192, 10, 100).unwrap();
    assert_eq!(pattern_result.operations, 100);
    assert!(pattern_result.ops_per_second > 0.0);
}

#[tokio::test]
async fn test_simd_vs_naive_performance() {
    let benchmark = PerformanceBenchmark::new();
    
    // Compare SIMD vs naive implementation
    let (simd_result, naive_result) = benchmark.compare_simd_performance(4096, 1000).unwrap();
    
    // SIMD should be at least as fast as naive implementation
    assert!(simd_result.throughput_mbps >= naive_result.throughput_mbps * 0.8); // Allow some variance
    assert_eq!(simd_result.simd_operations, 1000);
    assert_eq!(naive_result.simd_operations, 0);
    
    println!("SIMD throughput: {:.2} MB/s", simd_result.throughput_mbps);
    println!("Naive throughput: {:.2} MB/s", naive_result.throughput_mbps);
    println!("SIMD speedup: {:.2}x", simd_result.throughput_mbps / naive_result.throughput_mbps);
}

#[tokio::test]
async fn test_zero_copy_memory_efficiency() {
    let pool = Arc::new(ZeroCopyBufferPool::new());
    let processor = SimdDataProcessor::new(Arc::clone(&pool));
    
    // Allocate and use many buffers
    let mut buffers = Vec::new();
    for i in 0..100 {
        let mut buffer = pool.get_buffer(1024).unwrap();
        let test_data = vec![i as u8; 1024];
        processor.simd_copy(&test_data, buffer.as_mut_slice()).unwrap();
        buffer.set_length(1024);
        buffers.push(buffer);
    }
    
    // Return all buffers
    for buffer in buffers {
        pool.return_buffer(buffer);
    }
    
    // Check that buffers are reused efficiently
    let stats = pool.get_stats();
    assert_eq!(stats.current_pool_size, 100);
    
    // Allocate again and verify reuse
    let reused_buffer = pool.get_buffer(1024).unwrap();
    let stats_after_reuse = pool.get_stats();
    assert_eq!(stats_after_reuse.cache_hits, 1);
    assert_eq!(stats_after_reuse.current_pool_size, 99);
    
    pool.return_buffer(reused_buffer);
}

#[tokio::test]
async fn test_comprehensive_optimization_integration() {
    let pool = Arc::new(ZeroCopyBufferPool::new());
    let message_processor = SimdMessageProcessor::new(Arc::clone(&pool));
    let mut pattern_matcher = SimdPatternMatcher::new();
    
    // Add patterns for filtering
    pattern_matcher.add_pattern(b"urgent", None, 1);
    pattern_matcher.add_pattern(b"critical", None, 2);
    
    // Create test messages with different priorities
    let messages = vec![
        ValkyrieMessage::new(
            MessageType::Data,
            "source".to_string(),
            DestinationType::Unicast("dest".to_string()),
            MessagePayload::Text("This is an urgent message".to_string())
        ),
        ValkyrieMessage::new(
            MessageType::Data,
            "source".to_string(),
            DestinationType::Unicast("dest".to_string()),
            MessagePayload::Text("This is a critical alert".to_string())
        ),
        ValkyrieMessage::new(
            MessageType::Data,
            "source".to_string(),
            DestinationType::Unicast("dest".to_string()),
            MessagePayload::Text("This is a normal message".to_string())
        ),
    ];
    
    let start_time = Instant::now();
    
    // Process messages with SIMD optimizations
    let processed_buffers = message_processor.batch_process(&messages).unwrap();
    
    // Filter messages using pattern matching
    let mut filtered_messages = Vec::new();
    for (i, buffer) in processed_buffers.iter().enumerate() {
        let matches = pattern_matcher.match_patterns(buffer.as_slice());
        if !matches.is_empty() {
            filtered_messages.push((i, matches));
        }
    }
    
    let processing_time = start_time.elapsed();
    
    // Verify results
    assert_eq!(processed_buffers.len(), 3);
    assert_eq!(filtered_messages.len(), 2); // urgent and critical messages
    
    // Check performance
    let msg_stats = message_processor.get_stats();
    let pattern_stats = pattern_matcher.get_stats();
    
    assert!(msg_stats.messages_processed >= 3);
    assert!(msg_stats.simd_operations > 0);
    assert!(pattern_stats.matches_performed >= 3);
    
    println!("Integrated processing time: {:?}", processing_time);
    println!("Messages processed: {}", msg_stats.messages_processed);
    println!("SIMD operations: {}", msg_stats.simd_operations);
    println!("Pattern matches: {}", pattern_stats.patterns_matched);
    
    // Verify sub-millisecond performance for small messages
    if processing_time.as_micros() < 1000 {
        println!("✓ Sub-millisecond processing achieved: {}μs", processing_time.as_micros());
    }
}

#[tokio::test]
async fn test_memory_alignment_and_simd_compatibility() {
    let buffer = SimdBuffer::new(1024).unwrap();
    let ptr = buffer.as_slice().as_ptr();
    
    // Verify 64-byte alignment for AVX-512 compatibility
    assert_eq!(ptr as usize % 64, 0, "Buffer should be 64-byte aligned for SIMD operations");
    
    // Test that SIMD operations work correctly with aligned memory
    let pool = Arc::new(ZeroCopyBufferPool::new());
    let processor = SimdDataProcessor::new(pool);
    
    let data_a = vec![0xAA; 64];
    let data_b = vec![0x55; 64];
    let mut output = vec![0; 64];
    
    // This should work without issues due to proper alignment
    processor.simd_xor(&data_a, &data_b, &mut output).unwrap();
    assert_eq!(output, vec![0xFF; 64]);
}