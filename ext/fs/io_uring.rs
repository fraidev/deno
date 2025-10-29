// Copyright 2018-2025 the Deno authors. MIT license.

//! io_uring support for Deno file system operations.
//!
//! This module provides high-performance asynchronous file I/O using Linux's io_uring
//! interface. io_uring is only available on Linux kernel >= 5.6.

use std::borrow::Cow;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;

use deno_io::fs::File;
use deno_io::fs::FsResult;
use deno_io::fs::FsStat;
use deno_permissions::CheckedPath;
use deno_permissions::CheckedPathBuf;
use tokio_uring::fs::File as IoUringFile;

use crate::FileSystem;
use crate::OpenOptions;
use crate::RealFs;
use crate::interface::FsDirEntry;
use crate::interface::FsFileType;

/// Minimum required Linux kernel version for io_uring support.
const MIN_KERNEL_VERSION: (u32, u32) = (5, 6);

/// Static flag indicating whether io_uring is available on this system.
static IO_URING_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Checks if io_uring is available on the current system.
///
/// Returns `true` if:
/// - Running on Linux
/// - Kernel version >= 5.6
/// - io_uring feature is enabled at compile time
pub fn is_io_uring_available() -> bool {
  *IO_URING_AVAILABLE.get_or_init(|| {
    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    {
      check_kernel_version()
    }
    #[cfg(not(all(target_os = "linux", feature = "io_uring")))]
    {
      false
    }
  })
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn check_kernel_version() -> bool {
  use std::fs;

  // Read kernel version from /proc/sys/kernel/osrelease
  let version_str = match fs::read_to_string("/proc/sys/kernel/osrelease") {
    Ok(s) => s,
    Err(_) => return false,
  };

  parse_kernel_version(&version_str)
    .map(|(major, minor)| {
      (major, minor) >= MIN_KERNEL_VERSION
    })
    .unwrap_or(false)
}

/// Parses a kernel version string like "5.10.0-1-amd64" or "6.1.0".
/// Returns (major, minor) version numbers.
fn parse_kernel_version(version_str: &str) -> Option<(u32, u32)> {
  let version_str = version_str.trim();

  // Split by '.' to get version components
  let mut parts = version_str.split('.');

  let major = parts.next()?.parse::<u32>().ok()?;
  let minor = parts.next()?
    .split('-')  // Handle versions like "10-1-amd64"
    .next()?
    .parse::<u32>()
    .ok()?;

  Some((major, minor))
}

/// Initialize io_uring support if available.
///
/// This should be called early in the runtime initialization.
/// Returns true if io_uring was successfully initialized.
pub fn init_io_uring() -> bool {
  is_io_uring_available()
}

/// Helper to read a file using io_uring.
///
/// NOTE: This is a proof-of-concept implementation that shows how io_uring
/// can be used. Full integration requires running tokio-uring in a dedicated
/// runtime context.
pub async fn read_file_with_io_uring(
  path: impl AsRef<std::path::Path>,
) -> std::io::Result<Vec<u8>> {
  // Open the file
  let file = IoUringFile::open(path).await?;

  // Get file size for buffer allocation
  let metadata = file.statx().await?;
  let size = metadata.stx_size as usize;

  // Read the entire file
  let (result, buf) = file.read_at(vec![0u8; size], 0).await;
  result?;

  Ok(buf)
}

/// Helper to write a file using io_uring.
///
/// NOTE: This is a proof-of-concept implementation that shows how io_uring
/// can be used. Full integration requires running tokio-uring in a dedicated
/// runtime context.
pub async fn write_file_with_io_uring(
  path: impl AsRef<std::path::Path>,
  data: Vec<u8>,
) -> std::io::Result<()> {
  // Create/truncate the file
  let file = IoUringFile::create(path).await?;

  // Write all data
  let (result, _) = file.write_at(data, 0).await;
  result?;

  // Ensure data is flushed
  file.sync_all().await?;

  Ok(())
}

/// Helper to get file metadata using io_uring.
pub async fn stat_with_io_uring(
  path: impl AsRef<std::path::Path>,
) -> std::io::Result<std::fs::Metadata> {
  tokio_uring::fs::metadata(path).await
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_kernel_version() {
    assert_eq!(parse_kernel_version("5.10.0"), Some((5, 10)));
    assert_eq!(parse_kernel_version("5.6.0-1-amd64"), Some((5, 6)));
    assert_eq!(parse_kernel_version("6.1.0"), Some((6, 1)));
    assert_eq!(parse_kernel_version("5.4.0"), Some((5, 4)));
    assert_eq!(parse_kernel_version("4.19.0"), Some((4, 19)));

    // Edge cases
    assert_eq!(parse_kernel_version("5.10.0\n"), Some((5, 10)));
    assert_eq!(parse_kernel_version("  5.10.0  "), Some((5, 10)));

    // Invalid cases
    assert_eq!(parse_kernel_version("invalid"), None);
    assert_eq!(parse_kernel_version("5"), None);
    assert_eq!(parse_kernel_version(""), None);
  }

  #[test]
  fn test_version_comparison() {
    assert!((5, 6) >= MIN_KERNEL_VERSION);
    assert!((5, 10) >= MIN_KERNEL_VERSION);
    assert!((6, 0) >= MIN_KERNEL_VERSION);
    assert!((5, 5) < MIN_KERNEL_VERSION);
    assert!((4, 19) < MIN_KERNEL_VERSION);
  }
}
