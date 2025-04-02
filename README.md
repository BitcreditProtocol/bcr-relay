# Bitcredit Nostr Relay

A specialized Nostr relay implementation written in Rust for the Bitcredit application.

## Overview

This project provides a customized Nostr relay that powers the Bitcredit application, enabling decentralized, censorship-resistant communication with features specifically designed for credit-related transactions and interactions.

## Features

- **High-performance Rust implementation**: Built for speed, reliability, and memory safety
- **Bitcredit-specific event handling**: Custom event types and validation rules for credit-related operations
- **Scalable architecture**: Designed to handle high throughput of Nostr events
- **Authentication**: NIP-01, NIP-02, and custom authentication mechanisms
- **Data persistence**: Configurable storage backends for relay events
- **Filter optimization**: Enhanced query capabilities for Bitcredit-specific data

## Installation

### Prerequisites

- Rust (latest stable version)

### Building from source

```bash
# Clone the repository
git clone https://github.com/bitcredit/bcr-relay.git
cd bcr-relay

# Build the project
cargo build --release

# Run the relay
./target/release/bcr-relay
```

## Configuration

Configuration is handled via a `config.toml` file in the project root:

```toml
[server]
host = "127.0.0.1"
port = 8080

[relay]
name = "Bitcredit Relay"
description = "A specialized Nostr relay for Bitcredit"
```

## API Documentation

The relay implements the standard Nostr protocol (NIPs) with Bitcredit-specific extensions:

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Nostr Protocol](https://github.com/nostr-protocol/nostr)
- [Bitcredit Project](https://www.bit.cr/)
