# deno_fs

This crate provides ops for interacting with the file system.

## Features

### io_uring Support (Linux only)

Optional high-performance asynchronous file I/O using Linux's io_uring interface.

**Requirements:**
- Linux kernel ≥ 5.6
- Enable the `io_uring` feature flag during build

**Build with io_uring:**
```bash
cargo build --features io_uring
```

**Benefits:**
- 2-3x faster file operations
- 5-10x better performance for concurrent I/O
- Lower CPU usage and latency

See [docs/io_uring.md](../../docs/io_uring.md) for detailed documentation.
