# xaichat

**xaichat** is a distributed, high-performance client-server chat system built from the ground up using **Rust**. It supports real-time messaging, group chats, file transfers, and inline multimedia rendering.

> [!IMPORTANT]
> **Academic Disclaimer:** This is a laboratory project developed as part of the university curriculum at the **Igor Sikorsky Kyiv Polytechnic Institute**. Development was prioritized for speed and functionality to meet academic deadlines. Consequently, the codebase may contain "rough edges" or non-idiomatic patterns. **This repository is considered complete and will not be actively maintained or developed further.**

## 🚀 Features

- **Real-time Communication:** Low-latency messaging powered by asynchronous I/O.
- **Message Routing:** Support for Private (1-to-1), Group, and Broadcast (Global) channels.
- **Offline Persistence:** All messages are stored in a PostgreSQL database; clients automatically sync missed history upon login.
- **Multimedia & Files:** Support for any file type transfer.
- **Inline Image Rendering:** PNG, JPG, GIF, and WebP images are decoded and displayed directly in the chat.
- **Text Formatting:** Basic Markdown-like support (e.g., `**bold text**`).
- **Presence Tracking:** Real-time online/offline status indicators for all registered users.

## 🛠 Technology Stack

- **Language:** [Rust](https://www.rust-lang.org/) (Stable)
- **Asynchronous Runtime:** [Tokio](https://tokio.rs/)
- **GUI Framework:** [egui](https://github.com/emilk/egui) (Immediate Mode GUI)
- **Database:** [PostgreSQL](https://www.postgresql.org/) with [sqlx](https://github.com/launchbadge/sqlx) (Async driver)
- **Serialization:** [Postcard](https://github.com/jamesmunns/postcard) (Efficient binary format)
- **Protocol:** Custom length-delimited binary protocol over TCP.

## 🏗 Architectural Patterns

### 1. Actor Model (Concurrency)
The server avoids traditional heavy locking (Mutex/RwLock) for global state. Instead, it utilizes an **Actor Model** pattern. A centralized `Router` actor owns the state (sessions, group memberships), and communication with individual connection handlers happens via asynchronous **MPSC (Multiple Producer, Single Consumer)** channels. This prevents deadlocks and ensures high scalability.

### 2. Publish/Subscribe (Pub/Sub)
Group messaging is implemented as a Pub/Sub system. The `Router` acts as the broker, managing a map of topics (Group IDs) and their subscribers (User IDs).

### 3. Interior Mutability
The client-side multimedia cache utilizes `RefCell` to allow updating texture handles during the `egui` render loop, bypassing strict borrow-checker constraints while maintaining thread safety.

### 4. Graceful Shutdown & Network Robustness
To solve the "TCP Reset Race Condition," the server implements a graceful shutdown sequence. It ensures that error packets (like "Invalid Credentials") are fully flushed to the client before the socket is closed, preventing unexpected connection resets on the client-side.

## 📦 Project Structure

- `/server`: The core routing engine and database integration.
- `/client`: The graphical user interface and background network manager.
- `/protocol`: A shared crate containing the binary message definitions (Shared Schema).
- `/migrations`: SQL scripts for initializing the PostgreSQL schema.

## 🛠 Setup & Running

### Prerequisites
- [Rust toolchain](https://rustup.rs/)
- [PostgreSQL](https://www.postgresql.org/) instance

### Configuration
1. Create a `.env` file in the `server` directory:
   ```env
   DATABASE_URL=postgres://user:password@localhost/xaichat
   ```
2. Run database migrations:
   ```bash
   sqlx migrate run
   ```

### Running

1. Start the Server:
   ```bash
   cargo run --bin server
   ```

2. Start the Client:
   ```bash
   cargo run --bin client
   ```

## 📝 License

This project is for educational purposes. Feel free to reference the 
code, but please note the maintenance status mentioned above.