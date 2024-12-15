# Flocker

<!-- [![Crates.io](https://img.shields.io/crates/v/flocker.svg)](https://crates.io/crates/flocker)
[![Documentation](https://docs.rs/flocker/badge.svg)](https://docs.rs/flocker)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) -->

A CLI tool for managing Fluree Docker containers with ease. Flocker simplifies the process of running and managing Fluree instances through an interactive interface.

## Features

- 🐳 **Docker Management**

  - List and select from local or remote Fluree Docker images
  - Configure port mappings and data volume mounts
  - Start, stop, and manage containers

- 📊 **Ledger Management**

  - List all ledgers in a running Fluree instance
  - View detailed ledger information
  - Delete ledgers with safety confirmations

- 💾 **Data Persistence**

  - Mount local directories for data persistence
  - Save and restore user preferences
  - Remember last used configuration

- 🎨 **User Experience**
  - Interactive command-line interface
  - Colorful and clear output
  - Helpful error messages

## Requirements

- Rust 1.70 or later
- Docker Desktop or Docker Engine
- Internet connection (for pulling remote images)

## Installation

Install Flocker using GitHub:

```bash
cargo install --git https://github.com/aaj3f/flocker.git
```

## Usage

Simply run `flocker` in your terminal:

```bash
flocker
```

> You can see DEBUG logs by running `flocker --verbose`

### First Run

1. Choose between remote or local Fluree images
2. Select a specific image version
3. Configure port mapping (default: 8090)
4. Optionally mount a local directory for data persistence
5. Choose between foreground or background execution

### Managing Running Containers

When a Fluree container is running, Flocker provides options to:

- View container statistics
- View container logs
- List and manage ledgers
- Stop the container
- Stop and destroy the container

### Ledger Management

When viewing ledgers, you can:

- See ledger statistics (commit count, size, last update)
- View detailed ledger information
- Safely delete ledgers (with confirmation)

## Configuration

Flocker automatically saves your preferences in:

- macOS: `~/Library/Application Support/com.fluree.flocker/config.json`
- Linux: `~/.config/flocker/config.json`
- Windows: `%APPDATA%\fluree\flocker\config.json`

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
