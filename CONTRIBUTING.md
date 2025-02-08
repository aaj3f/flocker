# Contributing to Flocker

Flocker is a CLI tool for managing Fluree Docker containers. This guide will help you understand the codebase organization and how to extend its functionality.

## Project Structure

```
flocker/
├── src/
│   ├── cli.rs         # CLI argument parsing
│   ├── config.rs      # Container configuration
│   ├── error.rs       # Error types and handling
│   ├── lib.rs         # Core types and re-exports
│   ├── main.rs        # Main application entry point
│   ├── state.rs       # Application state management
│   ├── docker/        # Docker operations
│   │   ├── manager.rs # Docker API interactions
│   │   ├── mod.rs     # Module exports
│   │   └── types.rs   # Docker-related types
│   └── ui/           # User interface components
│       ├── container.rs # Container management UI
│       ├── image.rs    # Image selection UI
│       ├── ledger.rs   # Ledger management UI
│       └── mod.rs      # Module exports
└── tests/
    ├── container_workflow_test.rs # Container workflow tests
    └── integration_test.rs        # Integration tests
```

## Core Components

### 1. Docker Operations (`src/docker/`)

The `docker` module handles all interactions with the Docker daemon through the bollard API.

- `DockerOperations` trait (`manager.rs`): Defines the interface for Docker operations
- `DockerManager` struct: Implements Docker operations using bollard
- `types.rs`: Contains Docker-related type definitions
- Key operations include:
  - Container lifecycle (create, start, stop, remove)
  - Container inspection and status
  - Container stats and logs
  - Ledger management within containers
  - Image pulling and listing

To add new Docker operations:

1. Add the method to the `DockerOperations` trait
2. Implement it in `DockerManager` using bollard
3. Add a mock implementation in `tests/container_workflow_test.rs`

Example:

```rust
// In src/docker/manager.rs
#[async_trait]
pub trait DockerOperations {
    // Add new method to trait
    async fn new_operation(&self, param: &str) -> Result<String>;
}

impl DockerManager {
    // Implement the operation
    async fn new_operation(&self, param: &str) -> Result<String> {
        // Implementation using bollard
    }
}

// In tests/container_workflow_test.rs
#[async_trait]
impl DockerOperations for MockDockerManager {
    // Add mock implementation
    async fn new_operation(&self, _param: &str) -> Result<String> {
        Ok("mock result".to_string())
    }
}
```

### 2. User Interface (`src/ui/`)

The UI module provides interactive command-line interface components.

- `UserInterface` trait: Defines common UI operations (input, selection, display)
- Component-specific UIs:
  - `ContainerUI`: Container management and configuration interface
  - `ImageUI`: Image selection and pulling interface
  - `LedgerUI`: Ledger viewing and management interface
- Uses dialoguer and console crates for CLI interactions

To add new UI capabilities:

1. Add methods to the appropriate UI component
2. Update the mock UI implementation in tests if needed
3. Use the new UI methods in main.rs

Example:

```rust
// In src/ui/container.rs
impl<UI: UserInterface> ContainerUI<UI> {
    // Add new UI method
    pub fn new_ui_method(&self) -> Result<String> {
        self.ui.get_string_input("Prompt message")
    }
}
```

### 3. State Management (`src/state.rs`)

The state module handles persistent application state and configuration.

- `State` struct: Manages persistent application state
- `ContainerInfo`: Container metadata including ID, name, port, data directory
- `DataDirConfig`: Data directory configuration with absolute/relative path handling
- Handles:
  - Loading/saving state to disk
  - Container tracking and updates
  - Default settings management
  - Configuration file location management

To extend state capabilities:

1. Add new fields to the relevant structs
2. Update serialization/deserialization if needed
3. Add methods for managing the new state

### 4. Configuration and CLI (`src/cli.rs`, `src/config.rs`)

- `cli.rs`: Handles command-line argument parsing and high-level CLI interactions

  - Tag management and formatting
  - Container action handling
  - User interaction flows
  - Remote image handling

- `config.rs`: Manages Fluree container configuration
  - Port mapping configuration
  - Volume mount handling
  - Container run mode settings
  - Configuration validation

### 5. Error Handling (`src/error.rs`)

- Custom error types for different failure scenarios:
  - Docker operation errors
  - Configuration errors
  - File operation errors
  - User input errors
- Error conversion implementations
- Structured error reporting

### 6. Main Application (`src/main.rs`)

The main module orchestrates all components:

1. Initializes logging and parses CLI arguments
2. Creates and manages Docker connections
3. Handles state persistence
4. Implements the main application loop:
   - Container selection/creation
   - Status monitoring
   - Action processing
   - State updates

To add new functionality:

1. Implement the required Docker operations
2. Add UI methods for user interaction
3. Update the main loop to handle new actions

## Adding New Features

When adding new features:

1. **Docker Operations**

   - Add methods to `DockerOperations` trait
   - Implement in `DockerManager` using bollard
   - Add mock implementation for tests

2. **User Interface**

   - Add UI methods to appropriate component
   - Update mock UI if needed
   - Keep UI separate from business logic

3. **Testing**

   - Add unit tests for new functionality
   - Update integration tests if needed
   - Ensure mock implementations cover new features

4. **Documentation**
   - Update inline documentation
   - Add examples if helpful
   - Update this guide if adding new patterns

## Testing

Run tests with:

```bash
cargo test
```

The test suite includes:

- Unit tests within each module
- Integration tests in `tests/`
- Mock implementations for testing UI and Docker operations

## Error Handling

- Use the `Result` type with `FlockerError`
- Add new error variants to `error.rs` if needed
- Provide descriptive error messages

## Code Style

- Follow Rust standard formatting (use `cargo fmt`)
- Add documentation for public items
- Use meaningful variable names
- Keep functions focused and modular

## Pull Request Process

1. Create a feature branch
2. Add tests for new functionality
3. Update documentation
4. Run the test suite
5. Submit PR with description of changes

## Questions?

If you have questions about the codebase or need help, please open an issue on GitHub.
