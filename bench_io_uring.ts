#!/usr/bin/env -S deno run --allow-read --allow-write
// Benchmark comparing Deno file operations with and without io_uring
//
// Usage:
// 1. Build Deno WITHOUT io_uring:
//    cargo build --release
//    ./target/release/deno run --allow-read --allow-write bench_io_uring.ts
//
// 2. Build Deno WITH io_uring (on Linux >= 5.6):
//    cargo build --release --features io_uring
//    ./target/release/deno run --allow-read --allow-write bench_io_uring.ts
//
// Compare the results between the two builds!

const SIZES = [
  { name: "1KB", size: 1024 },
  { name: "4KB", size: 4 * 1024 },
  { name: "16KB", size: 16 * 1024 },
  { name: "64KB", size: 64 * 1024 },
  { name: "256KB", size: 256 * 1024 },
  { name: "1MB", size: 1024 * 1024 },
  { name: "4MB", size: 4 * 1024 * 1024 },
];

const ITERATIONS = 100;
const CONCURRENT_OPS = 10;

console.log("╔════════════════════════════════════════════════════════════════╗");
console.log("║              Deno File I/O Performance Benchmark               ║");
console.log("╚════════════════════════════════════════════════════════════════╝\n");

// Print system info
console.log("System Information:");
console.log(`  Deno version: ${Deno.version.deno}`);
console.log(`  V8 version: ${Deno.version.v8}`);
console.log(`  TypeScript version: ${Deno.version.typescript}`);
console.log(`  OS: ${Deno.build.os}`);
console.log(`  Arch: ${Deno.build.arch}`);

if (Deno.build.os === "linux") {
  try {
    const kernelVersion = await Deno.readTextFile("/proc/sys/kernel/osrelease");
    console.log(`  Kernel: ${kernelVersion.trim()}`);
    const [major, minor] = kernelVersion.split(".").map(Number);
    if (major > 5 || (major === 5 && minor >= 6)) {
      console.log("  ✓ Kernel supports io_uring (>= 5.6)");
    } else {
      console.log("  ✗ Kernel does NOT support io_uring (< 5.6)");
    }
  } catch {
    console.log("  Kernel: unknown");
  }
}
console.log();

console.log("Note: Build with --features io_uring to enable io_uring on Linux");
console.log("      Without the feature, spawn_blocking will be used\n");

// Create test directory
const testDir = "./bench_deno_tmp";
try {
  await Deno.mkdir(testDir);
} catch {
  // Directory might already exist
}

console.log("═══════════════════════════════════════════════════════════════");
console.log("  Single File Operations");
console.log("═══════════════════════════════════════════════════════════════\n");

for (const { name, size } of SIZES) {
  console.log(`Testing ${name} files (${ITERATIONS} iterations):`);

  // Benchmark write
  const writeTime = await benchWrite(size, ITERATIONS, testDir);
  console.log(`  Write: ${writeTime.toFixed(2)} ms avg`);

  // Benchmark read
  const readTime = await benchRead(size, ITERATIONS, testDir);
  console.log(`  Read:  ${readTime.toFixed(2)} ms avg`);

  // Benchmark stat
  const statTime = await benchStat(ITERATIONS, testDir);
  console.log(`  Stat:  ${statTime.toFixed(2)} ms avg`);

  // Cleanup
  await cleanup(testDir);
  console.log();
}

console.log("═══════════════════════════════════════════════════════════════");
console.log(`  Concurrent File Operations (${CONCURRENT_OPS} concurrent ops)`);
console.log("═══════════════════════════════════════════════════════════════\n");

for (const { name, size } of SIZES) {
  console.log(`Testing ${name} files (${CONCURRENT_OPS} concurrent):`);

  const concurrentTime = await benchConcurrent(size, CONCURRENT_OPS, testDir);
  console.log(`  Concurrent: ${concurrentTime.toFixed(2)} ms total`);
  console.log(`  Per operation: ${(concurrentTime / CONCURRENT_OPS).toFixed(2)} ms avg`);

  await cleanup(testDir);
  console.log();
}

console.log("═══════════════════════════════════════════════════════════════");
console.log("  Real-World Scenario: Processing Multiple Files");
console.log("═══════════════════════════════════════════════════════════════\n");

const scenarioTime = await benchRealWorldScenario(testDir);
console.log(`Processing time: ${scenarioTime.toFixed(2)} ms`);

// Final cleanup
await cleanup(testDir);
try {
  await Deno.remove(testDir);
} catch {
  // Ignore
}

console.log("\n╔════════════════════════════════════════════════════════════════╗");
console.log("║                    Benchmark Complete                          ║");
console.log("╚════════════════════════════════════════════════════════════════╝");

console.log("\nTo compare with io_uring:");
console.log("1. Build with: cargo build --release --features io_uring");
console.log("2. Run again:  ./target/release/deno run --allow-read --allow-write bench_io_uring.ts");
console.log("3. Compare the numbers - io_uring should be 2-3x faster!");

// Benchmark Functions

async function benchWrite(size: number, iterations: number, dir: string): Promise<number> {
  const data = new Uint8Array(size);

  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    await Deno.writeFile(`${dir}/write_${i}.tmp`, data);
  }
  const elapsed = performance.now() - start;

  return elapsed / iterations;
}

async function benchRead(size: number, iterations: number, dir: string): Promise<number> {
  // Setup: create test files
  const data = new Uint8Array(size);
  for (let i = 0; i < iterations; i++) {
    await Deno.writeFile(`${dir}/read_${i}.tmp`, data);
  }

  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    await Deno.readFile(`${dir}/read_${i}.tmp`);
  }
  const elapsed = performance.now() - start;

  return elapsed / iterations;
}

async function benchStat(iterations: number, dir: string): Promise<number> {
  // Setup: create a test file
  await Deno.writeFile(`${dir}/stat.tmp`, new Uint8Array([1, 2, 3]));

  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    await Deno.stat(`${dir}/stat.tmp`);
  }
  const elapsed = performance.now() - start;

  return elapsed / iterations;
}

async function benchConcurrent(size: number, concurrent: number, dir: string): Promise<number> {
  const data = new Uint8Array(size);

  const start = performance.now();
  const promises = [];
  for (let i = 0; i < concurrent; i++) {
    const promise = (async () => {
      const path = `${dir}/concurrent_${i}.tmp`;
      await Deno.writeFile(path, data);
      await Deno.readFile(path);
      await Deno.stat(path);
    })();
    promises.push(promise);
  }
  await Promise.all(promises);
  const elapsed = performance.now() - start;

  return elapsed;
}

async function benchRealWorldScenario(dir: string): Promise<number> {
  // Simulate a real-world scenario: processing multiple configuration files
  const files = [
    { name: "config.json", content: JSON.stringify({ app: "deno", version: "1.0" }) },
    { name: "settings.json", content: JSON.stringify({ theme: "dark", lang: "en" }) },
    { name: "data.json", content: JSON.stringify({ items: Array(100).fill({ id: 1, value: "test" }) }) },
    { name: "cache.json", content: JSON.stringify({ cache: Array(1000).fill("x".repeat(100)) }) },
  ];

  const start = performance.now();

  // Write all files
  await Promise.all(
    files.map((file) =>
      Deno.writeTextFile(`${dir}/${file.name}`, file.content)
    )
  );

  // Read and process all files
  const contents = await Promise.all(
    files.map((file) => Deno.readTextFile(`${dir}/${file.name}`))
  );

  // Parse JSON
  contents.forEach((content) => JSON.parse(content));

  // Check all files exist
  await Promise.all(
    files.map((file) => Deno.stat(`${dir}/${file.name}`))
  );

  const elapsed = performance.now() - start;
  return elapsed;
}

async function cleanup(dir: string) {
  try {
    for await (const entry of Deno.readDir(dir)) {
      if (entry.isFile && entry.name.endsWith(".tmp")) {
        await Deno.remove(`${dir}/${entry.name}`);
      }
    }
    // Also clean up JSON files from real-world scenario
    for (const name of ["config.json", "settings.json", "data.json", "cache.json"]) {
      try {
        await Deno.remove(`${dir}/${name}`);
      } catch {
        // Ignore
      }
    }
  } catch {
    // Directory might not exist or be empty
  }
}
