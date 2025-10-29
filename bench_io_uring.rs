// Copyright 2018-2025 the Deno authors. MIT license.

//! Benchmark comparing io_uring vs spawn_blocking for file operations.
//!
//! Build with:
//! ```bash
//! cargo build --release --bin bench_io_uring --features io_uring
//! ```
//!
//! Run on Linux with kernel >= 5.6:
//! ```bash
//! ./target/release/bench_io_uring
//! ```

use std::fs;
use std::io::Write;
use std::time::Instant;

#[cfg(all(target_os = "linux", feature = "io_uring"))]
use tokio_uring;

const TEST_SIZES: &[(&str, usize)] = &[
  ("1KB", 1024),
  ("4KB", 4 * 1024),
  ("16KB", 16 * 1024),
  ("64KB", 64 * 1024),
  ("256KB", 256 * 1024),
  ("1MB", 1024 * 1024),
  ("4MB", 4 * 1024 * 1024),
];

const ITERATIONS: usize = 100;
const CONCURRENT_OPS: usize = 10;

fn main() {
  println!("╔════════════════════════════════════════════════════════════════╗");
  println!("║         Deno io_uring vs spawn_blocking Benchmark             ║");
  println!("╚════════════════════════════════════════════════════════════════╝\n");

  // Check system info
  print_system_info();

  #[cfg(all(target_os = "linux", feature = "io_uring"))]
  {
    println!("✓ io_uring feature is enabled\n");

    // Check if io_uring is available at runtime
    let kernel_version = get_kernel_version();
    println!("Kernel version: {}", kernel_version);

    if check_io_uring_available() {
      println!("✓ io_uring is available\n");
      println!("Running benchmarks...\n");

      run_all_benchmarks();
    } else {
      println!("✗ io_uring is NOT available (requires kernel >= 5.6)");
      println!("Running spawn_blocking benchmarks only...\n");
      run_spawn_blocking_benchmarks_only();
    }
  }

  #[cfg(not(all(target_os = "linux", feature = "io_uring")))]
  {
    println!("✗ io_uring feature is not enabled or not on Linux");
    println!("Build with: cargo build --release --features io_uring\n");
    println!("Running spawn_blocking benchmarks only...\n");
    run_spawn_blocking_benchmarks_only();
  }

  println!("\n╔════════════════════════════════════════════════════════════════╗");
  println!("║                    Benchmark Complete                          ║");
  println!("╚════════════════════════════════════════════════════════════════╝");
}

fn print_system_info() {
  println!("System Information:");
  println!("  OS: {}", std::env::consts::OS);
  println!("  Arch: {}", std::env::consts::ARCH);
  println!("  CPUs: {}", num_cpus::get());
  println!();
}

fn get_kernel_version() -> String {
  #[cfg(target_os = "linux")]
  {
    fs::read_to_string("/proc/sys/kernel/osrelease")
      .unwrap_or_else(|_| "unknown".to_string())
      .trim()
      .to_string()
  }
  #[cfg(not(target_os = "linux"))]
  {
    "N/A (not Linux)".to_string()
  }
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn check_io_uring_available() -> bool {
  use std::fs;

  let version_str = match fs::read_to_string("/proc/sys/kernel/osrelease") {
    Ok(s) => s,
    Err(_) => return false,
  };

  parse_kernel_version(&version_str)
    .map(|(major, minor)| (major, minor) >= (5, 6))
    .unwrap_or(false)
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn parse_kernel_version(version_str: &str) -> Option<(u32, u32)> {
  let version_str = version_str.trim();
  let mut parts = version_str.split('.');
  let major = parts.next()?.parse::<u32>().ok()?;
  let minor = parts.next()?.split('-').next()?.parse::<u32>().ok()?;
  Some((major, minor))
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn run_all_benchmarks() {
  println!("═══════════════════════════════════════════════════════════════");
  println!("  Single File Operations");
  println!("═══════════════════════════════════════════════════════════════\n");

  for (size_name, size) in TEST_SIZES {
    println!("Testing {} files ({} iterations):", size_name, ITERATIONS);

    // Benchmark write operations
    let spawn_write = bench_spawn_blocking_write(*size, ITERATIONS);
    let uring_write = bench_io_uring_write(*size, ITERATIONS);

    print_comparison("  Write", spawn_write, uring_write);

    // Benchmark read operations
    let spawn_read = bench_spawn_blocking_read(*size, ITERATIONS);
    let uring_read = bench_io_uring_read(*size, ITERATIONS);

    print_comparison("  Read ", spawn_read, uring_read);

    // Benchmark stat operations
    let spawn_stat = bench_spawn_blocking_stat(ITERATIONS);
    let uring_stat = bench_io_uring_stat(ITERATIONS);

    print_comparison("  Stat ", spawn_stat, uring_stat);

    println!();
  }

  println!("═══════════════════════════════════════════════════════════════");
  println!("  Concurrent File Operations ({} concurrent ops)", CONCURRENT_OPS);
  println!("═══════════════════════════════════════════════════════════════\n");

  for (size_name, size) in TEST_SIZES {
    println!("Testing {} files ({} concurrent):", size_name, CONCURRENT_OPS);

    let spawn_concurrent = bench_spawn_blocking_concurrent(*size, CONCURRENT_OPS);
    let uring_concurrent = bench_io_uring_concurrent(*size, CONCURRENT_OPS);

    print_comparison("  Concurrent", spawn_concurrent, uring_concurrent);
    println!();
  }

  cleanup_test_files();
}

fn run_spawn_blocking_benchmarks_only() {
  println!("═══════════════════════════════════════════════════════════════");
  println!("  spawn_blocking Baseline Performance");
  println!("═══════════════════════════════════════════════════════════════\n");

  for (size_name, size) in TEST_SIZES {
    println!("Testing {} files ({} iterations):", size_name, ITERATIONS);

    let spawn_write = bench_spawn_blocking_write(*size, ITERATIONS);
    println!("  Write: {:.2} ms avg", spawn_write);

    let spawn_read = bench_spawn_blocking_read(*size, ITERATIONS);
    println!("  Read:  {:.2} ms avg", spawn_read);

    let spawn_stat = bench_spawn_blocking_stat(ITERATIONS);
    println!("  Stat:  {:.2} ms avg", spawn_stat);

    println!();
  }

  cleanup_test_files();
}

fn print_comparison(operation: &str, spawn_blocking_ms: f64, io_uring_ms: f64) {
  let speedup = spawn_blocking_ms / io_uring_ms;
  let diff_percent = ((spawn_blocking_ms - io_uring_ms) / spawn_blocking_ms) * 100.0;

  let indicator = if speedup > 1.2 {
    "🚀"
  } else if speedup > 1.0 {
    "✓"
  } else if speedup > 0.9 {
    "≈"
  } else {
    "⚠"
  };

  println!(
    "{}: spawn_blocking: {:>8.2}ms | io_uring: {:>8.2}ms | {:>6.2}x faster {:>6.1}% {} ",
    operation, spawn_blocking_ms, io_uring_ms, speedup, diff_percent, indicator
  );
}

// Benchmark spawn_blocking write
fn bench_spawn_blocking_write(size: usize, iterations: usize) -> f64 {
  let data = vec![0u8; size];
  let runtime = tokio::runtime::Runtime::new().unwrap();

  let start = Instant::now();
  for i in 0..iterations {
    let data_clone = data.clone();
    let path = format!("bench_spawn_write_{}.tmp", i);
    runtime.block_on(async move {
      tokio::task::spawn_blocking(move || {
        std::fs::write(&path, data_clone)
      })
      .await
      .unwrap()
      .unwrap();
    });
  }
  let elapsed = start.elapsed();

  elapsed.as_secs_f64() * 1000.0 / iterations as f64
}

// Benchmark spawn_blocking read
fn bench_spawn_blocking_read(size: usize, iterations: usize) -> f64 {
  // Setup: create test files
  let data = vec![0u8; size];
  for i in 0..iterations {
    let path = format!("bench_spawn_read_{}.tmp", i);
    std::fs::write(&path, &data).unwrap();
  }

  let runtime = tokio::runtime::Runtime::new().unwrap();

  let start = Instant::now();
  for i in 0..iterations {
    let path = format!("bench_spawn_read_{}.tmp", i);
    runtime.block_on(async move {
      tokio::task::spawn_blocking(move || {
        std::fs::read(&path)
      })
      .await
      .unwrap()
      .unwrap()
    });
  }
  let elapsed = start.elapsed();

  elapsed.as_secs_f64() * 1000.0 / iterations as f64
}

// Benchmark spawn_blocking stat
fn bench_spawn_blocking_stat(iterations: usize) -> f64 {
  // Setup: create a test file
  std::fs::write("bench_spawn_stat.tmp", b"test").unwrap();

  let runtime = tokio::runtime::Runtime::new().unwrap();

  let start = Instant::now();
  for _ in 0..iterations {
    runtime.block_on(async {
      tokio::task::spawn_blocking(|| {
        std::fs::metadata("bench_spawn_stat.tmp")
      })
      .await
      .unwrap()
      .unwrap()
    });
  }
  let elapsed = start.elapsed();

  elapsed.as_secs_f64() * 1000.0 / iterations as f64
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn bench_io_uring_write(size: usize, iterations: usize) -> f64 {
  let data = vec![0u8; size];

  let start = Instant::now();
  for i in 0..iterations {
    let data_clone = data.clone();
    let path = format!("bench_uring_write_{}.tmp", i);
    tokio_uring::start(async move {
      let file = tokio_uring::fs::File::create(&path).await.unwrap();
      let (result, _) = file.write_at(data_clone, 0).await;
      result.unwrap();
      file.sync_all().await.unwrap();
    });
  }
  let elapsed = start.elapsed();

  elapsed.as_secs_f64() * 1000.0 / iterations as f64
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn bench_io_uring_read(size: usize, iterations: usize) -> f64 {
  // Setup: create test files
  let data = vec![0u8; size];
  for i in 0..iterations {
    let path = format!("bench_uring_read_{}.tmp", i);
    std::fs::write(&path, &data).unwrap();
  }

  let start = Instant::now();
  for i in 0..iterations {
    let path = format!("bench_uring_read_{}.tmp", i);
    tokio_uring::start(async move {
      let file = tokio_uring::fs::File::open(&path).await.unwrap();
      let metadata = file.statx().await.unwrap();
      let size = metadata.stx_size as usize;
      let buf = vec![0u8; size];
      let (result, _) = file.read_at(buf, 0).await;
      result.unwrap();
    });
  }
  let elapsed = start.elapsed();

  elapsed.as_secs_f64() * 1000.0 / iterations as f64
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn bench_io_uring_stat(iterations: usize) -> f64 {
  // Setup: create a test file
  std::fs::write("bench_uring_stat.tmp", b"test").unwrap();

  let start = Instant::now();
  for _ in 0..iterations {
    tokio_uring::start(async {
      tokio_uring::fs::metadata("bench_uring_stat.tmp").await.unwrap()
    });
  }
  let elapsed = start.elapsed();

  elapsed.as_secs_f64() * 1000.0 / iterations as f64
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn bench_spawn_blocking_concurrent(size: usize, concurrent: usize) -> f64 {
  let data = vec![0u8; size];
  let runtime = tokio::runtime::Runtime::new().unwrap();

  let start = Instant::now();
  runtime.block_on(async {
    let mut handles = vec![];
    for i in 0..concurrent {
      let data_clone = data.clone();
      let path = format!("bench_spawn_concurrent_{}.tmp", i);
      let handle = tokio::task::spawn_blocking(move || {
        std::fs::write(&path, data_clone).unwrap();
        std::fs::read(&path).unwrap()
      });
      handles.push(handle);
    }
    for handle in handles {
      handle.await.unwrap();
    }
  });
  let elapsed = start.elapsed();

  elapsed.as_secs_f64() * 1000.0
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn bench_io_uring_concurrent(size: usize, concurrent: usize) -> f64 {
  let data = vec![0u8; size];

  let start = Instant::now();
  tokio_uring::start(async {
    let mut handles = vec![];
    for i in 0..concurrent {
      let data_clone = data.clone();
      let path = format!("bench_uring_concurrent_{}.tmp", i);
      let handle = tokio_uring::spawn(async move {
        // Write
        let file = tokio_uring::fs::File::create(&path).await.unwrap();
        let (result, _) = file.write_at(data_clone, 0).await;
        result.unwrap();
        file.sync_all().await.unwrap();
        drop(file);

        // Read
        let file = tokio_uring::fs::File::open(&path).await.unwrap();
        let metadata = file.statx().await.unwrap();
        let size = metadata.stx_size as usize;
        let buf = vec![0u8; size];
        let (result, buf) = file.read_at(buf, 0).await;
        result.unwrap();
        buf
      });
      handles.push(handle);
    }
    for handle in handles {
      handle.await.unwrap();
    }
  });
  let elapsed = start.elapsed();

  elapsed.as_secs_f64() * 1000.0
}

fn cleanup_test_files() {
  use std::fs;

  let _ = fs::remove_file("bench_spawn_stat.tmp");

  #[cfg(all(target_os = "linux", feature = "io_uring"))]
  {
    let _ = fs::remove_file("bench_uring_stat.tmp");
  }

  // Remove all test files
  for entry in fs::read_dir(".").unwrap() {
    let entry = entry.unwrap();
    let path = entry.path();
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
      if name.starts_with("bench_") && name.ends_with(".tmp") {
        let _ = fs::remove_file(&path);
      }
    }
  }
}

// Simple num_cpus implementation for systems that don't have the crate
mod num_cpus {
  pub fn get() -> usize {
    #[cfg(target_os = "linux")]
    {
      std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
    }
    #[cfg(not(target_os = "linux"))]
    {
      std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
    }
  }
}
