# Bitcredit Nostr Relay

A specialized Nostr relay implementation written in Rust for the Bitcredit application.

## Overview

This project provides a customized Nostr relay that powers the Bitcredit application, enabling decentralized, censorship-resistant communication with features specifically designed for credit-related transactions and interactions.

## Features

## Installation

### Prerequisites

- Rust (latest stable version)

### Building from source

```bash
# Clone the repository
git clone https://github.com/BitcreditProtocol/bcr-relay.git
cd bcr-relay

# Build the project
cargo build --release

# Run the relay
./target/release/bcr-relay
```

## API Documentation

The relay implements the standard Nostr protocol (NIPs) with Bitcredit-specific extensions:

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Nostr Protocol](https://github.com/nostr-protocol/nostr)
- [Bitcredit Project](https://www.bit.cr/)
