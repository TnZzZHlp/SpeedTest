# AGENTS.md - SpeedTest Rust Project

## Project Overview
SpeedTest is a Rust-based network speed testing tool that measures download speeds by testing multiple servers concurrently and using the fastest available server for continuous download speed measurement.

## Build Commands

### Development
```bash
# Build debug version
cargo build

# Build release version
cargo build --release

# Run the application
cargo run

# Run with specific arguments
cargo run -- --help                    # Show help
cargo run -- --url "https://example.com" --concurrency 8
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test (no tests currently exist, but pattern for future)
cargo test -- test_name

# Run tests with verbose output
cargo test -- --nocapture

# Test with specific features
cargo test -- --test-threads=1
```

### Code Quality
```bash
# Format code (no rustfmt configured but can be added)
cargo fmt

# Check for clippy warnings
cargo clippy

# Check for clippy warnings and fix automatically
cargo clippy --fix

# Check for common issues
cargo check

# Build documentation
cargo doc --open
```

### Release & Distribution
```bash
# Cross-compilation examples (from workflow)
cargo build --release --target x86_64-pc-windows-msvc
cargo build --release --target x86_64-unknown-linux-gnu

# Create release build with optimizations
cargo build --release
```

## Code Style Guidelines

### Imports
```rust
// Group imports by source with blank lines between groups
use clap::Parser;
use std::{
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};
use tokio::{spawn, task::JoinSet};

// Keep imports minimal - only import what's needed
// Use nested imports for related items from same module
```

### Formatting
- Use 4-space indentation (standard Rust)
- Line length: aim for <100 characters
- Braces on same line for functions, structs, enums
- Space after colon in type annotations
- Space around binary operators

### Types & Naming Conventions
```rust
// Structs and enums: PascalCase
struct Args {
    url: String,
    concurrency: usize,
}

// Variables and functions: snake_case
async fn downloader(client: Arc<reqwest::Client>, ua: String) {}

// Constants: SCREAMING_SNAKE_CASE
static ADDRESS: [&str; 10] = [...];
static SPEED: AtomicUsize = AtomicUsize::new(0);

// Type parameters: single uppercase letters (T, U, V) or PascalCase
```

### Error Handling
```rust
// Use Result for fallible operations
async fn test(address: String) -> (String, u128) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();  // Only use unwrap/expect in setup code
    
    match client.get(&address).send().await {
        Ok(res) => {
            if res.status().is_success() {
                (address, now.elapsed().as_millis())
            } else {
                (address, u128::MAX)  // Use sentinel values for errors
            }
        }
        Err(_) => (address, u128::MAX),
    }
}

// Panic for unrecoverable errors only
if args.concurrency == 0 {
    panic!("线程数不合法");  // Chinese error messages in this project
}
```

### Async/Await Patterns
```rust
// Use #[tokio::main] for main function
#[tokio::main]
async fn main() {
    // Use Arc for shared ownership in async context
    let client = Arc::new(reqwest::Client::new());
    
    // Use JoinSet for managing multiple tasks
    let mut tasks = JoinSet::new();
    
    // Spawn tasks with spawn()
    spawn(downloader(client.clone(), args.ua.clone()));
}
```

### Unsafe Code Usage
```rust
// Minimize unsafe usage - currently only for static mut
static mut BEST: String = String::new();

// Always comment why unsafe is necessary
unsafe {
    BEST = output[0].0.clone();
}
```

### Documentation
```rust
/// Document public items with triple slashes
#[derive(Parser)]
struct Args {
    /// 下载地址
    #[clap(short, long, default_value = "")]
    url: String,

    /// 线程数
    #[clap(short, long, default_value = "16")]
    concurrency: usize,
}
```

## Project Structure
- `src/main.rs`: Main application entry point
- `Cargo.toml`: Dependency management and project metadata
- `.github/workflows/build.yaml`: CI/CD for multi-platform builds

## Dependencies
- `clap`: Command-line argument parsing
- `reqwest`: HTTP client for downloads
- `tokio`: Async runtime for concurrent operations

## Development Workflow

1. **Make changes** to `src/main.rs`
2. **Test locally** with `cargo run -- --concurrency 4`
3. **Check for errors** with `cargo check`
4. **Run clippy** for linting: `cargo clippy`
5. **Build release** for testing: `cargo build --release`
6. **Commit changes** with descriptive messages

## Testing Strategy
- Currently no unit tests exist
- Future tests should be added in `src/` or `tests/` directory
- Integration tests for download functionality
- Mock network responses for reliable testing

## Performance Considerations
- Use atomic operations for thread-safe counters
- Minimize allocations in hot paths
- Use streaming downloads with chunks
- Configure timeouts for network operations

## Cross-Platform Notes
- Project supports Windows and Linux builds
- Uses Chinese error messages (consider localization)
- Network operations should handle platform-specific differences

## Agent Instructions
1. **Before making changes**: Run `cargo check` to ensure no compilation errors
2. **After making changes**: Run `cargo clippy` to check for common issues
3. **Test thoroughly**: Run the application with various arguments
4. **Follow existing patterns**: Match the code style and patterns in main.rs
5. **Add tests**: When implementing new features, consider adding unit tests
6. **Document unsafe code**: Always comment why unsafe blocks are necessary
7. **Handle errors gracefully**: Use Result and sentinel values rather than panics
8. **Optimize for async**: Use appropriate async patterns and avoid blocking

## Common Issues & Solutions
- **Compilation errors**: Run `cargo clean && cargo build`
- **Network timeouts**: Adjust timeout durations in test() function
- **Memory usage**: Monitor with streaming chunk downloads
- **Cross-compilation**: Use targets from workflow file

## Repository Information
- Language: Rust
- Edition: 2021
- Async Runtime: Tokio
- Build System: Cargo
- CI: GitHub Actions