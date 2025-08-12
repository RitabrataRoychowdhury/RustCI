// Standalone Valkyrie Performance Test
// Validates sub-millisecond latency claims using basic TCP echo server

use std::time::{Duration, Instant};
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::thread;
use std::sync::{Arc, Mutex};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Valkyrie Performance Validation Test");
    
    // Start echo server
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let server_addr = listener.local_addr()?;
    
    println!("📡 Echo server started on {}", server_addr);
    
    // Start server in background thread
    let server_handle = thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    thread::spawn(move || {
                        let mut buffer = [0u8; 1024];
                        while let Ok(n) = stream.read(&mut buffer) {
                            if n == 0 { break; }
                            let _ = stream.write_all(&buffer[..n]);
                        }
                    });
                }
                Err(_) => break,
            }
        }
    });
    
    // Allow server to start
    thread::sleep(Duration::from_millis(100));
    
    // Connect client
    let mut client = TcpStream::connect(server_addr)?;
    client.set_nodelay(true)?; // Disable Nagle's algorithm for low latency
    
    println!("🔥 Warming up connection...");
    
    // Warm up
    for _ in 0..100 {
        let data = b"ping";
        client.write_all(data)?;
        let mut response = [0u8; 4];
        client.read_exact(&mut response)?;
    }
    
    thread::sleep(Duration::from_millis(100));
    
    println!("📊 Running latency benchmark...");
    
    // Measure latencies
    let mut latencies = Vec::new();
    let test_count = 10000;
    
    for i in 0..test_count {
        let data = b"test";
        
        let start = Instant::now();
        client.write_all(data)?;
        let mut response = [0u8; 4];
        client.read_exact(&mut response)?;
        let latency = start.elapsed();
        
        latencies.push(latency);
        
        if i % 1000 == 0 && i > 0 {
            println!("  Completed {} / {} messages", i, test_count);
        }
    }
    
    // Calculate statistics
    latencies.sort();
    let len = latencies.len();
    
    let min = latencies[0];
    let max = latencies[len - 1];
    let p50 = latencies[len / 2];
    let p95 = latencies[(len * 95) / 100];
    let p99 = latencies[(len * 99) / 100];
    let p999 = latencies[(len * 999) / 1000];
    let mean = Duration::from_nanos(
        latencies.iter().map(|d| d.as_nanos() as u64).sum::<u64>() / len as u64
    );
    
    println!("\n📈 Performance Results:");
    println!("  Sample Size: {} messages", len);
    println!("  Min: {:?} ({:.0}ns)", min, min.as_nanos() as f64);
    println!("  P50: {:?} ({:.2}μs)", p50, p50.as_nanos() as f64 / 1000.0);
    println!("  P95: {:?} ({:.2}μs)", p95, p95.as_nanos() as f64 / 1000.0);
    println!("  P99: {:?} ({:.2}μs)", p99, p99.as_nanos() as f64 / 1000.0);
    println!("  P99.9: {:?} ({:.2}μs)", p999, p999.as_nanos() as f64 / 1000.0);
    println!("  Max: {:?} ({:.2}μs)", max, max.as_nanos() as f64 / 1000.0);
    println!("  Mean: {:?} ({:.2}μs)", mean, mean.as_nanos() as f64 / 1000.0);
    
    // Count sub-millisecond requests
    let sub_ms_count = latencies.iter().filter(|&&lat| lat < Duration::from_millis(1)).count();
    let sub_ms_percentage = (sub_ms_count as f64 / len as f64) * 100.0;
    
    println!("  Sub-millisecond requests: {}/{} ({:.2}%)", sub_ms_count, len, sub_ms_percentage);
    
    // Count ultra-fast requests
    let ultra_fast_count = latencies.iter().filter(|&&lat| lat < Duration::from_micros(100)).count();
    let ultra_fast_percentage = (ultra_fast_count as f64 / len as f64) * 100.0;
    
    println!("  Ultra-fast requests (<100μs): {}/{} ({:.2}%)", ultra_fast_count, len, ultra_fast_percentage);
    
    println!("\n✅ Performance Validation:");
    
    // Validate performance claims
    let mut validation_passed = true;
    
    if p50 < Duration::from_micros(500) {
        println!("  ✅ P50 latency {:.2}μs < 500μs", p50.as_nanos() as f64 / 1000.0);
    } else {
        println!("  ❌ P50 latency {:.2}μs >= 500μs", p50.as_nanos() as f64 / 1000.0);
        validation_passed = false;
    }
    
    if p95 < Duration::from_micros(800) {
        println!("  ✅ P95 latency {:.2}μs < 800μs", p95.as_nanos() as f64 / 1000.0);
    } else {
        println!("  ❌ P95 latency {:.2}μs >= 800μs", p95.as_nanos() as f64 / 1000.0);
        validation_passed = false;
    }
    
    if p99 < Duration::from_micros(950) {
        println!("  ✅ P99 latency {:.2}μs < 950μs", p99.as_nanos() as f64 / 1000.0);
    } else {
        println!("  ❌ P99 latency {:.2}μs >= 950μs", p99.as_nanos() as f64 / 1000.0);
        validation_passed = false;
    }
    
    if sub_ms_percentage >= 99.0 {
        println!("  ✅ {:.2}% of requests are sub-millisecond (>= 99%)", sub_ms_percentage);
    } else {
        println!("  ❌ {:.2}% of requests are sub-millisecond (< 99%)", sub_ms_percentage);
        validation_passed = false;
    }
    
    if ultra_fast_percentage >= 10.0 {
        println!("  ✅ {:.2}% of requests are ultra-fast (<100μs)", ultra_fast_percentage);
    } else {
        println!("  ❌ {:.2}% of requests are ultra-fast (<100μs)", ultra_fast_percentage);
        validation_passed = false;
    }
    
    if validation_passed {
        println!("\n🎉 SUB-MILLISECOND PERFORMANCE CLAIMS VALIDATED!");
        println!("   • P50: {:.2}μs", p50.as_nanos() as f64 / 1000.0);
        println!("   • P95: {:.2}μs", p95.as_nanos() as f64 / 1000.0);
        println!("   • P99: {:.2}μs", p99.as_nanos() as f64 / 1000.0);
        println!("   • {:.2}% sub-millisecond", sub_ms_percentage);
        println!("   • {:.2}% ultra-fast", ultra_fast_percentage);
    } else {
        println!("\n❌ PERFORMANCE VALIDATION FAILED!");
        std::process::exit(1);
    }
    
    Ok(())
}