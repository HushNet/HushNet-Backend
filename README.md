# 🔐 HushNet Backend

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-14%2B-blue.svg)](https://www.postgresql.org/)

**HushNet Backend** is a secure backend server for an end-to-end encrypted (E2EE) instant messaging application, built with Rust and Axum. It implements the Signal Protocol (X3DH + Double Ratchet) to provide cryptographically secure communication between users and devices.

---

## 📋 Table of Contents

- [🔐 HushNet Backend](#-hushnet-backend)
  - [📋 Table of Contents](#-table-of-contents)
  - [✨ Features](#-features)
  - [🚀 Quick Start](#-quick-start)
  - [📚 Documentation](#-documentation)
    - [Core Documentation](#core-documentation)
    - [Setup \& Deployment](#setup--deployment)
    - [Additional Resources](#additional-resources)
  - [🏗️ Architecture](#️-architecture)
    - [Communication Flow](#communication-flow)
  - [🛠️ Technology Stack](#️-technology-stack)
    - [Core Framework](#core-framework)
    - [Cryptography](#cryptography)
    - [Database](#database)
    - [Utilities](#utilities)
  - [📋 Prerequisites](#-prerequisites)
  - [⚙️ Installation](#️-installation)
    - [Quick Install](#quick-install)
  - [🔧 Configuration](#-configuration)
    - [Environment Variables](#environment-variables)
  - [🤝 Contributing](#-contributing)
    - [Quick Start for Contributors](#quick-start-for-contributors)
  - [📄 License](#-license)
  - [🙏 Acknowledgments](#-acknowledgments)
  - [📞 Contact](#-contact)

---

## ✨ Features

- **🔒 End-to-End Encryption (E2EE)**: Signal Protocol implementation (X3DH + Double Ratchet)
- **🔐 Ed25519 Authentication**: Cryptographic signature-based authentication (no JWT)
- **👥 Multi-Device Support**: Full support for multiple devices per user
- **💬 Instant Messaging**: Direct messages and group chats
- **🔑 Cryptographic Key Management**: Identity keys, prekeys, signed prekeys, one-time prekeys
- **⚡ Real-time Communication**: WebSockets for instant notifications
- **📡 PostgreSQL LISTEN/NOTIFY**: Real-time event notifications
- **🚀 High Performance**: Asynchronous backend powered by Tokio
- **🐳 Docker Ready**: Docker configuration for PostgreSQL
- **🛡️ Anti-Replay Protection**: Timestamp-based request validation

---

## 🚀 Quick Start

```bash
# Clone the repository
git clone https://github.com/HushNet/HushNet-Backend.git
cd HushNet-Backend

# Start PostgreSQL with Docker
docker build -t hushnet-postgres .
docker run -d -p 5432:5432 --name hushnet-db hushnet-postgres

# Configure environment
echo "DATABASE_URL=postgres://postgres:dev@localhost:5432/e2ee" > .env

# Build and run
cargo build --release
cargo run
```

Server will start at `http://127.0.0.1:8080` 🎉

---

## 📚 Documentation

Comprehensive documentation is organized into the following sections:

### Core Documentation

- **[API Reference](docs/API.md)** - Complete API endpoint documentation
- **[Database Schema](docs/DATABASE.md)** - Database structure and relationships
- **[Security & Cryptography](docs/SECURITY.md)** - Encryption protocols and authentication
- **[WebSocket & Real-time](docs/REALTIME.md)** - WebSocket implementation and events
- **[Project Structure](docs/STRUCTURE.md)** - Code organization and architecture

### Setup & Deployment

- **[Installation Guide](docs/INSTALLATION.md)** - Detailed setup instructions
- **[Configuration Guide](docs/CONFIGURATION.md)** - Environment variables and settings
- **[Docker Deployment](docs/DOCKER.md)** - Docker and Docker Compose setup
- **[Development Guide](docs/DEVELOPMENT.md)** - Development workflow and tools

### Additional Resources

- **[Contributing Guidelines](docs/CONTRIBUTING.md)** - How to contribute to the project
- **[Changelog](docs/CHANGELOG.md)** - Version history and updates
- **[Roadmap](docs/ROADMAP.md)** - Future features and improvements

---

## 🏗️ Architecture

HushNet Backend follows a clean, modular architecture:

```
┌─────────────────────────────────────────────────────────┐
│                    Client Applications                   │
└───────────────────────┬─────────────────────────────────┘
                        │ HTTP/WebSocket
┌───────────────────────▼─────────────────────────────────┐
│                   Axum Web Framework                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │   Routes    │  │ Middlewares │  │ Controllers │    │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘    │
│         │                 │                 │            │
│         └────────┬────────┴────────┬────────┘           │
│                  │                 │                     │
│         ┌────────▼─────────┐  ┌───▼─────────┐          │
│         │    Services      │  │ Repositories │          │
│         └──────────────────┘  └───────┬──────┘          │
└───────────────────────────────────────┼─────────────────┘
                                        │
                    ┌───────────────────▼──────────────────┐
                    │       PostgreSQL Database            │
                    │  (LISTEN/NOTIFY for real-time)       │
                    └──────────────────────────────────────┘
```

### Communication Flow

1. **Authentication**: Ed25519 signature verification on each request
2. **Key Exchange**: X3DH handshake to establish secure sessions
3. **Messaging**: Double Ratchet encryption for each message
4. **Real-time**: PostgreSQL NOTIFY → WebSocket → Clients

---

## 🛠️ Technology Stack

### Core Framework

- **[Rust](https://www.rust-lang.org/)** (Edition 2021) - Programming language
- **[Axum](https://github.com/tokio-rs/axum)** 0.7 - Async web framework
- **[Tokio](https://tokio.rs/)** - Async runtime
- **[SQLx](https://github.com/launchbadge/sqlx)** 0.8 - PostgreSQL async client

### Cryptography

- **[ed25519-dalek](https://github.com/dalek-cryptography/ed25519-dalek)** 2.0 - Digital signatures
- **[base64](https://github.com/marshallpierce/rust-base64)** - Base64 encoding/decoding

### Database

- **[PostgreSQL](https://www.postgresql.org/)** 14+ - Relational database
- **LISTEN/NOTIFY** - Real-time notification mechanism

### Utilities

- **[Serde](https://serde.rs/)** - JSON serialization/deserialization
- **[Tracing](https://github.com/tokio-rs/tracing)** - Structured logging
- **[Dotenvy](https://github.com/allan2/dotenvy)** - Environment variable management
- **[Anyhow](https://github.com/dtolnay/anyhow)** - Error handling
- **[UUID](https://github.com/uuid-rs/uuid)** - Unique identifier generation
- **[Chrono](https://github.com/chronotope/chrono)** - Date and time handling

---

## 📋 Prerequisites

- **Rust** 1.70+ ([Install](https://www.rust-lang.org/tools/install))
- **PostgreSQL** 14+ ([Install](https://www.postgresql.org/download/))
- **Docker** (optional, for database)
- **Cargo** (included with Rust)

---

## ⚙️ Installation

For detailed installation instructions, see the [Installation Guide](docs/INSTALLATION.md).

### Quick Install

```bash
# 1. Clone repository
git clone https://github.com/HushNet/HushNet-Backend.git
cd HushNet-Backend

# 2. Install dependencies
cargo build

# 3. Setup database (with Docker)
docker build -t hushnet-postgres .
docker run -d -p 5432:5432 --name hushnet-db hushnet-postgres

# 4. Configure environment
cp .env.example .env
# Edit .env with your configuration

# 5. Run server
cargo run
```

---

## 🔧 Configuration

### Environment Variables

Create a `.env` file in the project root:

```env
# Database
DATABASE_URL=postgres://postgres:dev@localhost:5432/e2ee

# Logging
RUST_LOG=info
```

For complete configuration options, see the [Configuration Guide](docs/CONFIGURATION.md).

---

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guidelines](docs/CONTRIBUTING.md) for details.

### Quick Start for Contributors

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Format code (`cargo fmt`)
6. Check lints (`cargo clippy`)
7. Commit changes (`git commit -m 'Add amazing feature'`)
8. Push to branch (`git push origin feature/amazing-feature`)
9. Open a Pull Request

---

## 📄 License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- [Signal Protocol](https://signal.org/docs/) for cryptographic inspiration
- [Axum](https://github.com/tokio-rs/axum) for the excellent web framework
- The Rust community 🦀

---

## 📞 Contact

- **GitHub Issues**: [Open an issue](https://github.com/HushNet/HushNet-Backend/issues)
- **GitHub Discussions**: [Join the discussion](https://github.com/HushNet/HushNet-Backend/discussions)

---

<div align="center">

**Built with ❤️ and 🦀 Rust**

⭐ If you like this project, please give it a star!

[Documentation](docs/) • [API Reference](docs/API.md) • [Contributing](docs/CONTRIBUTING.md)

</div>
