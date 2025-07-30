# Rust Code Examples

This directory contains Rust code examples and standalone tests that demonstrate various RustCI features and implementations.

## Current Examples

### `event_driven_test.rs`
A standalone test demonstrating the event-driven system enhancements implemented in RustCI, including:

- Event sourcing with projections
- Command queuing and batching
- Authorization checks for commands
- Retry mechanisms with backoff
- Schema evolution support
- Command history and audit tracking
- Event-driven architecture with handlers
- Correlation tracking across operations

## Running Examples

To run the event-driven test example:

```bash
cargo run --bin event_driven_test
```

Or compile and run directly:

```bash
rustc rust-examples/event_driven_test.rs --extern tokio --extern serde --extern serde_json --extern uuid --extern chrono -o event_driven_test
./event_driven_test
```

## Adding New Examples

When adding new code examples:

1. Create a new `.rs` file in this directory
2. Add appropriate documentation comments
3. Include usage examples in the file header
4. Update this README with a description
5. Ensure the example is self-contained and runnable

## Purpose

These code examples serve as:

- **Learning resources** for developers using RustCI
- **Integration tests** for specific features
- **Reference implementations** for advanced patterns
- **Standalone demonstrations** of core functionality

## Directory Structure

```
rust-examples/
├── README.md                 # This file
├── event_driven_test.rs      # Event-driven system demonstration
└── [future examples]         # Additional Rust code examples
```

## Related Documentation

- **Pipeline Configuration Examples**: See `docs/pipeline-examples/` for YAML configuration examples
- **Test Configurations**: See `tests/` for connector and integration test configurations
- **API Documentation**: See `docs/api/` for REST API examples
- **Architecture Guide**: See `docs/architecture.md` for system design patterns