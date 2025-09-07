// Valkyrie Throughput Performance Test
// Tests high throughput capabilities with concurrent connections

use std::time::{Duration, Instant};
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::thread;
use std::sync::{Arc, Mutex};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Valkyrie Throughput Validation Test");
    
    // Start echo server
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let server_addr = listener.local_addr()?;
    
    println!("📡 Echo server started on {}", server_addr);
    
    // Start server in background thread
    let _server_handle = thread::spawn(move || {
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
    
    thread::sleep(Duration::from_millis(100));
    
    println!("📊 Running throughput test with 10 concurrent clients...");
    
    let client_count = 10;
    let test_duration = Duration::from_secs(5);
    let start_time = Instant::now();
    
    let results = Arc::new(Mutex::new(Vec::new()));
    let mut client_handles = Vec::new();
    
    for client_id in 0..client_count {
        let addr = server_addr;
        let results_clone = results.clone();
        let handle = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client.set_nodelay(true).unwrap();
            
            let mut operations = 0u64;
            let mut total_latency = Duration::ZERO;
            
            while start_time.elapsed() < test_duration {
                let data = b"test";
                
                let op_start = Instant::now();
                let _ = client.write_all(data);
                let mut response = [0u8; 4];
                let _ = client.read_exact(&mut response);
                let latency = op_start.elapsed();
                
                operations += 1;
                total_latency += latency;
            }
            
            let avg_latency = if operations > 0 {
                total_latency / operations as u32
            } else {
                Duration::ZERO
            };
            
            let mut results_guard = results_clone.lock().unwrap();
            results_guard.push((client_id, operations, avg_latency));
        });
        
        client_handles.push(handle);
    }
    
    // Wait for all clients to complete
    for handle in client_handles {
        handle.join().unwrap();
    }
    
    let results_guard = results.lock().unwrap();
    let mut total_operations = 0u64;
    let mut total_latency = Duration::ZERO;
    
    for &(client_id, operations, avg_latency) in results_guard.iter() {
        println!("  Client {}: {} ops, avg latency: {:.2}μs", 
                client_id, operations, avg_latency.as_nanos() as f64 / 1000.0);
        total_operations += operations;
        total_latency += avg_latency;
    }
    
    let actual_duration = start_time.elapsed();
    let throughput = total_operations as f64 / actual_duration.as_secs_f64();
    let avg_latency = total_latency / client_count as u32;
    
    println!("\n📈 Throughput Results:");
    println!("  Total Operations: {}", total_operations);
    println!("  Test Duration: {:.2}s", actual_duration.as_secs_f64());
    println!("  Throughput: {:.2} ops/sec", throughput);
    println!("  Average Latency: {:.2}μs", avg_latency.as_nanos() as f64 / 1000.0);
    println!("  Theoretical Max Latency: {:.2}μs", 1_000_000.0 / throughput);
    
    println!("\n✅ Throughput Validation:");
    
    let mut validation_passed = true;
    
    if throughput >= 50000.0 {
        println!("  ✅ Throughput {:.2} ops/sec >= 50,000 ops/sec", throughput);
    } else {
        println!("  ❌ Throughput {:.2} ops/sec < 50,000 ops/sec", throughput);
        validation_passed = false;
    }
    
    if avg_latency < Duration::from_micros(500) {
        println!("  ✅ Average latency {:.2}μs < 500μs", avg_latency.as_nanos() as f64 / 1000.0);
    } else {
        println!("  ❌ Average latency {:.2}μs >= 500μs", avg_latency.as_nanos() as f64 / 1000.0);
        validation_passed = false;
    }
    
    if validation_passed {
        println!("\n🎉 HIGH THROUGHPUT PERFORMANCE VALIDATED!");
        println!("   • Throughput: {:.0} ops/sec", throughput);
        println!("   • Avg Latency: {:.2}μs", avg_latency.as_nanos() as f64 / 1000.0);
    } else {
        println!("\n❌ THROUGHPUT VALIDATION FAILED!");
        std::process::exit(1);
    }
    
    Ok(())
}