# 📁 Project Structure

Complete guide to HushNet Backend code organization.

---

## Directory Tree

```
HushNet-Backend/
├── src/                          # Source code
│   ├── main.rs                   # Application entry point
│   ├── app_state.rs             # Shared application state
│   │
│   ├── controllers/             # HTTP request handlers
│   │   ├── mod.rs
│   │   ├── chats_controller.rs
│   │   ├── device_controller.rs
│   │   ├── keys_controller.rs
│   │   ├── messages_controller.rs
│   │   ├── root_controller.rs
│   │   ├── session_controller.rs
│   │   └── user_controller.rs
│   │
│   ├── routes/                  # Route definitions
│   │   ├── mod.rs
│   │   ├── chats.rs
│   │   ├── devices.rs
│   │   ├── messages.rs
│   │   ├── root.rs
│   │   ├── sessions.rs
│   │   ├── users.rs
│   │   └── websocket.rs
│   │
│   ├── repository/              # Data access layer
│   │   ├── mod.rs
│   │   ├── chat_repository.rs
│   │   ├── device_repository.rs
│   │   ├── enrollment_token_repository.rs
│   │   ├── keys_repository.rs
│   │   ├── message_repository.rs
│   │   ├── session_repository.rs
│   │   └── user_repository.rs
│   │
│   ├── services/                # Business logic
│   │   ├── mod.rs
│   │   └── auth.rs
│   │
│   ├── models/                  # Data structures
│   │   ├── mod.rs
│   │   ├── chat.rs
│   │   ├── device.rs
│   │   ├── enrollment_token.rs
│   │   ├── keys.rs
│   │   ├── message.rs
│   │   ├── realtime.rs
│   │   ├── session.rs
│   │   └── user.rs
│   │
│   ├── middlewares/             # HTTP middlewares
│   │   ├── mod.rs
│   │   └── auth.rs
│   │
│   ├── realtime/                # Real-time communication
│   │   ├── mod.rs
│   │   ├── listener.rs         # PostgreSQL LISTEN
│   │   └── websocket.rs        # WebSocket handlers
│   │
│   ├── utils/                   # Utility functions
│   │   ├── mod.rs
│   │   └── crypto_utils.rs
│   │
│   └── db/                      # Database utilities
│       ├── mod.rs
│       ├── connection.rs
│       └── models.rs
│
├── sql_models/                   # Database schemas
│   └── seed.sql
│
├── docs/                         # Documentation
│   ├── API.md
│   ├── DATABASE.md
│   ├── SECURITY.md
│   ├── REALTIME.md
│   ├── STRUCTURE.md
│   ├── INSTALLATION.md
│   ├── CONFIGURATION.md
│   ├── DOCKER.md
│   ├── DEVELOPMENT.md
│   ├── CONTRIBUTING.md
│   ├── ROADMAP.md
│   └── CHANGELOG.md
│
├── target/                       # Build artifacts (ignored)
├── Cargo.toml                    # Rust dependencies
├── Cargo.lock                    # Dependency lock file
├── Dockerfile                    # PostgreSQL Docker image
├── .env                          # Environment variables (not committed)
├── .gitignore                    # Git ignore rules
├── README.md                     # Main documentation
└── LICENSE                       # MIT License
```

---

## Layer Architecture

```
┌─────────────────────────────────────────────┐
│              HTTP/WebSocket                  │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│              Routes Layer                    │
│  (URL mapping, parameter extraction)         │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│           Middleware Layer                   │
│  (Authentication, logging, CORS)             │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│           Controllers Layer                  │
│  (Request handling, response formatting)     │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│            Services Layer                    │
│  (Business logic, validation)                │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│          Repository Layer                    │
│  (Database operations, queries)              │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│          Database (PostgreSQL)               │
└─────────────────────────────────────────────┘
```

---

## Module Descriptions

### `main.rs`

Entry point of the application.

**Responsibilities**:
- Initialize logging
- Load environment variables
- Connect to PostgreSQL
- Start PostgreSQL listeners
- Setup routes and middlewares
- Start HTTP server

```rust
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // 1. Initialize tracing
    tracing_subscriber::fmt::init();
    
    // 2. Load .env
    dotenvy::dotenv().ok();
    
    // 3. Connect to database
    let pool = PgPool::connect(&database_url).await?;
    
    // 4. Setup real-time
    let (tx, _rx) = broadcast::channel::<RealtimeEvent>(100);
    tokio::spawn(start_pg_listeners(pool.clone(), tx.clone()));
    
    // 5. Build application
    let app = Router::new()
        .merge(routes::users::routes())
        .merge(routes::devices::routes())
        // ... more routes
        .layer(Extension(tx));
    
    // 6. Start server
    axum::serve(listener, app).await.unwrap();
    Ok(())
}
```

### `app_state.rs`

Shared application state passed to all handlers.

```rust
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: String,  // Note: Not currently used
}
```

### `routes/`

Defines URL routes and maps them to controllers.

**Example** (`routes/users.rs`):

```rust
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_users).post(create_user))
        .route("/users/:id", get(get_user_by_id))
        .route("/login", post(login_user))
}
```

### `controllers/`

Handles HTTP requests and returns responses.

**Responsibilities**:
- Extract request parameters
- Call services/repositories
- Format responses
- Handle errors

**Example** (`controllers/user_controller.rs`):

```rust
pub async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<User>, StatusCode> {
    let user = user_repository::create_user(&state.pool, &payload.username)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(user))
}
```

### `middlewares/`

Request/response interceptors.

**Authentication Middleware** (`middlewares/auth.rs`):

```rust
pub struct AuthenticatedDevice(pub Devices);

#[async_trait]
impl FromRequestParts<AppState> for AuthenticatedDevice {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // 1. Extract headers (X-Identity-Key, X-Signature, X-Timestamp)
        // 2. Verify timestamp (anti-replay)
        // 3. Verify Ed25519 signature
        // 4. Fetch device from database
        // 5. Return authenticated device
    }
}
```

**Usage in controllers**:

```rust
pub async fn protected_endpoint(
    AuthenticatedDevice(device): AuthenticatedDevice,
    State(state): State<AppState>,
) -> Response {
    // `device` is guaranteed to be authenticated
}
```

### `repository/`

Database access layer using SQLx.

**Responsibilities**:
- Execute SQL queries
- Map database rows to Rust structs
- Handle database errors

**Example** (`repository/user_repository.rs`):

```rust
pub async fn get_user_by_id(
    pool: &PgPool,
    user_id: &Uuid,
) -> Result<User, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT id, username, created_at FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}
```

### `services/`

Business logic and complex operations.

**Example** (`services/auth.rs`):

```rust
pub fn verify_signature(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), SignatureError> {
    let verifying_key = VerifyingKey::from_bytes(public_key)?;
    let sig = Signature::from_bytes(signature);
    verifying_key.verify(message, &sig)
}
```

### `models/`

Data structures and serialization.

**Example** (`models/user.rs`):

```rust
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
}
```

### `realtime/`

Real-time communication implementation.

**`listener.rs`**: PostgreSQL LISTEN

```rust
pub async fn start_pg_listeners(
    pool: PgPool,
    tx: broadcast::Sender<RealtimeEvent>,
) {
    tokio::spawn(async move {
        let mut listener = PgListener::connect_with(&pool).await?;
        listener.listen_all(vec![
            "messages_channel",
            "sessions_channel",
            "devices_channel",
        ]).await?;

        loop {
            while let Ok(Some(notif)) = listener.try_recv().await {
                let event: RealtimeEvent = serde_json::from_str(notif.payload())?;
                tx.send(event)?;
            }
        }
    });
}
```

**`websocket.rs`**: WebSocket handler

```rust
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    Extension(tx): Extension<broadcast::Sender<RealtimeEvent>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, params.user_id, tx))
}

async fn handle_socket(
    socket: WebSocket,
    user_id: Uuid,
    tx: broadcast::Sender<RealtimeEvent>,
) {
    let mut rx = tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if event.user_id == user_id {
                let json = serde_json::to_string(&event)?;
                sender.send(Message::Text(json)).await?;
            }
        }
    });
}
```

### `utils/`

Helper functions and utilities.

**Example** (`utils/crypto_utils.rs`):

```rust
pub fn decode_base64(input: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::engine::general_purpose::STANDARD.decode(input)
}

pub fn encode_base64(input: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(input)
}
```

---

## Data Flow Example

### Send Message Flow

```
1. Client → POST /messages
   Headers: X-Identity-Key, X-Signature, X-Timestamp
   Body: { chat_id, to_device_id, header, ciphertext }

2. routes/messages.rs
   → Maps to messages_controller::send_message

3. middlewares/auth.rs
   → Extracts AuthenticatedDevice
   → Verifies Ed25519 signature

4. controllers/messages_controller.rs
   → Extracts request body
   → Calls message_repository::create_message

5. repository/message_repository.rs
   → Executes INSERT query
   → Returns created message

6. PostgreSQL
   → messages_notify_trigger fires
   → NOTIFY messages_channel

7. realtime/listener.rs
   → Receives NOTIFY
   → Broadcasts to WebSocket handlers

8. realtime/websocket.rs
   → Filters by user_id
   → Sends to connected client

9. Client ← WebSocket message
   { type: "message", chat_id: "...", user_id: "..." }
```

---

## Module Dependencies

```
main.rs
  ├─ routes/* (all route modules)
  ├─ app_state.rs
  ├─ realtime/listener.rs
  └─ models/realtime.rs

routes/*
  ├─ controllers/*
  └─ app_state.rs

controllers/*
  ├─ repository/*
  ├─ services/*
  ├─ models/*
  ├─ middlewares/auth.rs
  └─ app_state.rs

middlewares/auth.rs
  ├─ models/device.rs
  ├─ repository/device_repository.rs
  ├─ utils/crypto_utils.rs
  └─ app_state.rs

repository/*
  ├─ models/*
  └─ sqlx

services/*
  ├─ models/*
  └─ utils/*

realtime/listener.rs
  ├─ models/realtime.rs
  └─ sqlx

realtime/websocket.rs
  ├─ models/realtime.rs
  └─ axum/extract/ws
```

---

## Testing Structure

```
src/
├─ controllers/
│  ├─ user_controller.rs
│  └─ user_controller_test.rs   # Unit tests
│
├─ repository/
│  ├─ user_repository.rs
│  └─ user_repository_test.rs   # Integration tests with test DB
│
└─ services/
   ├─ auth.rs
   └─ auth_test.rs              # Unit tests

tests/
├─ integration/
│  ├─ api_tests.rs              # Full API tests
│  └─ websocket_tests.rs        # WebSocket tests
│
└─ common/
   └─ mod.rs                     # Test utilities
```

---

## Best Practices

### Module Organization

1. **One concern per module**: Each module has a single responsibility
2. **Clear boundaries**: Layers don't skip levels (controller → repository OK, route → repository NOT OK)
3. **Minimal public API**: Expose only what's necessary
4. **Internal modules**: Use `mod` for module-local utilities

### Naming Conventions

- **Files**: `snake_case` (e.g., `user_controller.rs`)
- **Structs**: `PascalCase` (e.g., `AuthenticatedDevice`)
- **Functions**: `snake_case` (e.g., `get_user_by_id`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `MAX_RETRIES`)

### Error Handling

```rust
// Use Result types
pub async fn get_user(id: Uuid) -> Result<User, Error> {
    // ...
}

// Use anyhow for application errors
use anyhow::{Context, Result};

pub async fn complex_operation() -> Result<()> {
    let user = get_user(id)
        .await
        .context("Failed to fetch user")?;
    Ok(())
}
```

---

[← Back to Main Documentation](../README.md)
