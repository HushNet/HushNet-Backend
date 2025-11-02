# 🗄️ Database Schema

Complete database schema documentation for HushNet Backend.

---

## Table of Contents

- [Overview](#overview)
- [Entity Relationship Diagram](#entity-relationship-diagram)
- [Tables](#tables)
- [Triggers](#triggers)
- [Indexes](#indexes)
- [Views](#views)
- [Migrations](#migrations)
- [Best Practices](#best-practices)

---

## Overview

HushNet uses **PostgreSQL 14+** with the following design principles:

- **UUID primary keys** for distributed scalability
- **Foreign key constraints** for referential integrity
- **JSONB columns** for flexible cryptographic data storage
- **Triggers** for real-time event notifications (LISTEN/NOTIFY)
- **Timestamps** with timezone support

---

## Entity Relationship Diagram

```
┌─────────────┐         ┌──────────────┐         ┌─────────────┐
│   users     │◄────────│   devices    │────────►│used_tokens  │
│             │ 1     N │              │         │             │
│ - id        │         │ - id         │         │ - token     │
│ - username  │         │ - user_id    │         │ - used_at   │
│ - created_at│         │ - identity_  │         └─────────────┘
└──────┬──────┘         │   pubkey     │
       │                │ - prekeys    │
       │ 1              │ - last_seen  │
       │                └──────┬───────┘
       │                       │
       │                       │ N
       │                       │
       │ N             ┌───────▼────────┐
       ├───────────────►│ pending_      │
       │               │ sessions       │
       │               │                │
       │               │ - sender_      │
       │               │   device_id    │
       │ N             │ - recipient_   │
       │               │   device_id    │
       ├──────────┐    │ - ephemeral_   │
       │          │    │   pubkey       │
       │          │    │ - ciphertext   │
       │          │    │ - state        │
       │          │    └────────────────┘
       │          │
       │          │    ┌─────────────┐
       │          └───►│   chats     │◄────┐
       │     N         │             │     │
       │               │ - id        │     │ 1
       │               │ - chat_type │     │
       │               │ - user_a    │     │
       │               │ - user_b    │     │
       │               │ - name      │     │
       │               │ - owner_id  │     │
       │         N     │ - last_msg  │     │
       └───────────────►│   _id       │     │
                       └──────┬──────┘     │
                              │ 1          │
                              │            │
                       ┌──────▼──────┐     │
                       │  messages   │─────┘
                       │             │
                       │ - id        │
                       │ - logical_  │
                       │   msg_id    │
                       │ - chat_id   │
                       │ - from_     │
                       │   device_id │
                       │ - to_device │
                       │   _id       │
                       │ - header    │
                       │ - ciphertext│
                       │ - timestamps│
                       └─────────────┘
                       
                       ┌─────────────┐
                       │  sessions   │
                       │             │
                       │ - id        │
                       │ - chat_id   │
                       │ - sender_   │
                       │   device_id │
                       │ - receiver_ │
                       │   device_id │
                       └─────────────┘
                       
                       ┌─────────────┐
                       │chat_members │
                       │             │
                       │ - chat_id   │
                       │ - user_id   │
                       │ - role      │
                       │ - joined_at │
                       └─────────────┘
```

---

## Tables

### `users`

Stores user accounts.

```sql
CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  username TEXT UNIQUE NOT NULL,
  created_at TIMESTAMP DEFAULT NOW()
);
```

**Columns**:

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | UUID | PRIMARY KEY, DEFAULT | Unique user identifier |
| `username` | TEXT | UNIQUE, NOT NULL | User's unique username |
| `created_at` | TIMESTAMP | DEFAULT NOW() | Account creation timestamp |

**Indexes**:
- Primary key on `id`
- Unique index on `username`

---

### `devices`

Stores devices and their cryptographic keys.

```sql
CREATE TABLE devices (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  identity_pubkey TEXT NOT NULL,
  prekey_pubkey TEXT NOT NULL,
  signed_prekey_pub TEXT NOT NULL,
  signed_prekey_sig TEXT NOT NULL,
  one_time_prekeys JSONB NOT NULL,
  device_label TEXT,
  push_token TEXT,
  last_seen TIMESTAMP DEFAULT NOW(),
  created_at TIMESTAMP DEFAULT NOW(),
  UNIQUE (user_id, identity_pubkey)
);
```

**Columns**:

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | UUID | PRIMARY KEY | Unique device identifier |
| `user_id` | UUID | FK → users(id), CASCADE | Owner of the device |
| `identity_pubkey` | TEXT | NOT NULL | Ed25519 public key (base64) |
| `prekey_pubkey` | TEXT | NOT NULL | Curve25519 prekey public |
| `signed_prekey_pub` | TEXT | NOT NULL | Signed prekey public |
| `signed_prekey_sig` | TEXT | NOT NULL | Signature of signed prekey |
| `one_time_prekeys` | JSONB | NOT NULL | Array of one-time prekeys |
| `device_label` | TEXT | NULLABLE | User-friendly device name |
| `push_token` | TEXT | NULLABLE | Push notification token |
| `last_seen` | TIMESTAMP | DEFAULT NOW() | Last activity timestamp |
| `created_at` | TIMESTAMP | DEFAULT NOW() | Device registration time |

**Indexes**:
- Primary key on `id`
- Unique index on `(user_id, identity_pubkey)`
- Index on `user_id` (via FK)

**Triggers**:
- `devices_notify_trigger`: Notifies on INSERT/UPDATE

---

### `chats`

Stores conversations (direct or group).

```sql
CREATE TABLE chats (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  chat_type TEXT CHECK (chat_type IN ('direct','group')) DEFAULT 'direct',
  
  -- Direct chat participants
  user_a UUID REFERENCES users(id) ON DELETE CASCADE,
  user_b UUID REFERENCES users(id) ON DELETE CASCADE,
  
  -- Group chat info
  name TEXT,
  owner_id UUID REFERENCES users(id) ON DELETE SET NULL,
  
  -- Last message reference
  last_message_id UUID,
  
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  
  -- Constraint: direct chats must have both users set
  CONSTRAINT chats_direct_shape
    CHECK (
      chat_type <> 'direct'
      OR (user_a IS NOT NULL AND user_b IS NOT NULL AND user_a < user_b)
    )
);
```

**Columns**:

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | UUID | PRIMARY KEY | Unique chat identifier |
| `chat_type` | TEXT | CHECK (direct/group) | Type of conversation |
| `user_a` | UUID | FK → users(id), CASCADE | First participant (direct) |
| `user_b` | UUID | FK → users(id), CASCADE | Second participant (direct) |
| `name` | TEXT | NULLABLE | Group chat name |
| `owner_id` | UUID | FK → users(id), SET NULL | Group owner |
| `last_message_id` | UUID | FK → messages(id) | Most recent message |
| `created_at` | TIMESTAMP | DEFAULT NOW() | Chat creation time |
| `updated_at` | TIMESTAMP | DEFAULT NOW() | Last update time |

**Unique Indexes**:

```sql
CREATE UNIQUE INDEX uniq_direct_chat
  ON chats (LEAST(user_a, user_b), GREATEST(user_a, user_b))
  WHERE chat_type = 'direct';
```

This prevents duplicate direct chats like (Alice, Bob) and (Bob, Alice).

**Foreign Keys**:

```sql
ALTER TABLE chats
  ADD CONSTRAINT fk_chats_last_message
  FOREIGN KEY (last_message_id)
  REFERENCES messages(id)
  ON DELETE SET NULL;
```

---

### `chat_members`

Stores group chat membership.

```sql
CREATE TABLE chat_members (
  chat_id UUID REFERENCES chats(id) ON DELETE CASCADE,
  user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  role TEXT DEFAULT 'member',
  joined_at TIMESTAMP DEFAULT NOW(),
  PRIMARY KEY (chat_id, user_id)
);
```

**Columns**:

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `chat_id` | UUID | PK, FK → chats(id) | Chat identifier |
| `user_id` | UUID | PK, FK → users(id) | Member user ID |
| `role` | TEXT | DEFAULT 'member' | User role in group |
| `joined_at` | TIMESTAMP | DEFAULT NOW() | Join timestamp |

**Note**: Only used for group chats. Direct chats use `user_a` and `user_b` in the `chats` table.

---

### `messages`

Stores encrypted messages.

```sql
CREATE TABLE messages (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  
  -- Logical message ID (shared across device fanouts)
  logical_msg_id TEXT NOT NULL,
  
  chat_id UUID REFERENCES chats(id) ON DELETE CASCADE,
  
  -- Sender information
  from_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  from_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  
  -- Recipient information
  to_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  to_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  
  -- Encrypted content
  header JSONB NOT NULL,
  ciphertext TEXT NOT NULL,
  
  -- Status timestamps
  delivered_at TIMESTAMPTZ,
  read_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'UTC')
);
```

**Columns**:

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | UUID | PRIMARY KEY | Unique message instance ID |
| `logical_msg_id` | TEXT | NOT NULL | Shared ID for all device copies |
| `chat_id` | UUID | FK → chats(id) | Parent conversation |
| `from_user_id` | UUID | FK → users(id) | Sender user |
| `from_device_id` | UUID | FK → devices(id) | Sender device |
| `to_user_id` | UUID | FK → users(id) | Recipient user |
| `to_device_id` | UUID | FK → devices(id) | Recipient device |
| `header` | JSONB | NOT NULL | Double Ratchet header |
| `ciphertext` | TEXT | NOT NULL | Encrypted message content |
| `delivered_at` | TIMESTAMPTZ | NULLABLE | Delivery confirmation time |
| `read_at` | TIMESTAMPTZ | NULLABLE | Read receipt time |
| `created_at` | TIMESTAMPTZ | DEFAULT UTC NOW | Message creation time |

**Header JSONB Structure**:

```json
{
  "dh_pubkey": "base64_encoded_ratchet_public_key",
  "pn": 5,  // Previous chain length
  "n": 12   // Message number in current chain
}
```

**Indexes**:

```sql
CREATE INDEX idx_messages_todevice ON messages(to_device_id, created_at);
CREATE INDEX idx_messages_chatid ON messages(chat_id, created_at);
CREATE INDEX idx_messages_logical ON messages(logical_msg_id);
```

**Triggers**:
- `messages_notify_trigger`: Notifies on INSERT

---

### `sessions`

Stores Double Ratchet session metadata (NOT the keys themselves).

```sql
CREATE TABLE sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  chat_id UUID REFERENCES chats(id) ON DELETE CASCADE,
  sender_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  receiver_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  UNIQUE (sender_device_id, receiver_device_id)
);
```

**Columns**:

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | UUID | PRIMARY KEY | Session identifier |
| `chat_id` | UUID | FK → chats(id) | Associated chat |
| `sender_device_id` | UUID | FK → devices(id) | Initiator device |
| `receiver_device_id` | UUID | FK → devices(id) | Recipient device |
| `created_at` | TIMESTAMP | DEFAULT NOW() | Session establishment |
| `updated_at` | TIMESTAMP | DEFAULT NOW() | Last activity |

**Important**: This table stores only metadata. Actual cryptographic keys (root keys, chain keys, message keys) are NEVER stored on the server.

**Triggers**:
- `sessions_notify_trigger`: Notifies on INSERT

---

### `pending_sessions`

Stores X3DH handshake initialization attempts.

```sql
CREATE TABLE pending_sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  sender_device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  recipient_device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  ephemeral_pubkey TEXT NOT NULL,
  sender_prekey_pub TEXT NOT NULL,
  otpk_used TEXT NOT NULL,
  ciphertext TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT NOW(),
  state TEXT DEFAULT 'initiated' CHECK (state IN ('initiated','responded','completed'))
);
```

**Columns**:

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | UUID | PRIMARY KEY | Pending session ID |
| `sender_device_id` | UUID | FK → devices(id) | Initiator device |
| `recipient_device_id` | UUID | FK → devices(id) | Recipient device |
| `ephemeral_pubkey` | TEXT | NOT NULL | Sender's ephemeral key (EK_A) |
| `sender_prekey_pub` | TEXT | NOT NULL | Sender's identity key (IK_A) |
| `otpk_used` | TEXT | NOT NULL | One-time prekey consumed |
| `ciphertext` | TEXT | NOT NULL | Initial encrypted message |
| `created_at` | TIMESTAMP | DEFAULT NOW() | Handshake initiation time |
| `state` | TEXT | CHECK, DEFAULT 'initiated' | Handshake state |

**State Values**:
- `initiated`: Sender has started handshake
- `responded`: Recipient has acknowledged
- `completed`: Session established

**Indexes**:

```sql
CREATE INDEX idx_pending_sessions_recipient 
  ON pending_sessions(recipient_device_id, created_at);
```

---

### `used_tokens`

Prevents replay of one-time tokens (anti-replay table).

```sql
CREATE TABLE used_tokens (
  token TEXT PRIMARY KEY,
  used_at TIMESTAMP DEFAULT NOW()
);
```

**Columns**:

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `token` | TEXT | PRIMARY KEY | Used token value |
| `used_at` | TIMESTAMP | DEFAULT NOW() | Usage timestamp |

**Note**: Periodically clean old tokens (e.g., > 24 hours) to prevent unbounded growth.

---

## Triggers

### `messages_notify_trigger`

Sends PostgreSQL NOTIFY when a new message is inserted.

```sql
CREATE OR REPLACE FUNCTION notify_new_message() RETURNS trigger AS $$
BEGIN
  PERFORM pg_notify(
    'messages_channel',
    json_build_object(
      'type', 'message',
      'chat_id', NEW.chat_id,
      'user_id', NEW.to_user_id
    )::text
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER messages_notify_trigger
AFTER INSERT ON messages
FOR EACH ROW
EXECUTE FUNCTION notify_new_message();
```

**Purpose**: Real-time notification to recipients via WebSocket.

---

### `sessions_notify_trigger`

Sends PostgreSQL NOTIFY when a new session is established.

```sql
CREATE OR REPLACE FUNCTION notify_new_session() RETURNS trigger AS $$
DECLARE
  sender_user UUID;
  receiver_user UUID;
BEGIN
  -- Retrieve user IDs
  SELECT user_id INTO sender_user FROM devices WHERE id = NEW.sender_device_id;
  SELECT user_id INTO receiver_user FROM devices WHERE id = NEW.receiver_device_id;

  -- Notify receiver
  IF receiver_user IS NOT NULL THEN
    PERFORM pg_notify(
      'sessions_channel',
      json_build_object(
        'type', 'session',
        'user_id', receiver_user,
        'sender_device_id', NEW.sender_device_id,
        'receiver_device_id', NEW.receiver_device_id
      )::text
    );
  END IF;

  -- Notify sender
  IF sender_user IS NOT NULL THEN
    PERFORM pg_notify(
      'sessions_channel',
      json_build_object(
        'type', 'session',
        'user_id', sender_user,
        'sender_device_id', NEW.sender_device_id,
        'receiver_device_id', NEW.receiver_device_id
      )::text
    );
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER sessions_notify_trigger
AFTER INSERT ON sessions
FOR EACH ROW
EXECUTE FUNCTION notify_new_session();
```

**Purpose**: Notify both parties when a secure session is established.

---

### `devices_notify_trigger`

Sends PostgreSQL NOTIFY when a device is added or updated.

```sql
CREATE OR REPLACE FUNCTION notify_device_update() RETURNS trigger AS $$
BEGIN
  PERFORM pg_notify(
    'devices_channel',
    json_build_object('type', 'device', 'user_id', NEW.user_id)::text
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER devices_notify_trigger
AFTER UPDATE OR INSERT ON devices
FOR EACH ROW
EXECUTE FUNCTION notify_device_update();
```

**Purpose**: Notify users when new devices are registered or keys are updated.

---

## Indexes

### Performance Indexes

```sql
-- Messages: Fast lookup by recipient device
CREATE INDEX idx_messages_todevice ON messages(to_device_id, created_at);

-- Messages: Fast lookup by chat
CREATE INDEX idx_messages_chatid ON messages(chat_id, created_at);

-- Messages: Deduplication by logical ID
CREATE INDEX idx_messages_logical ON messages(logical_msg_id);

-- Pending sessions: Fast lookup by recipient
CREATE INDEX idx_pending_sessions_recipient 
  ON pending_sessions(recipient_device_id, created_at);
```

### Rationale

- **Composite indexes** (device_id, created_at) support both filtering and ordering
- **Logical message ID** allows fast deduplication checks
- **Pending sessions** optimized for "inbox" queries

---

## Views

### `user_devices_view`

Convenient view for fetching user devices with public keys.

```sql
CREATE VIEW user_devices_view AS
SELECT
  u.username,
  d.id AS device_id,
  d.identity_pubkey,
  d.signed_prekey_pub,
  d.signed_prekey_sig,
  (d.one_time_prekeys ->> 0) AS one_time_prekey_pub
FROM users u
JOIN devices d ON u.id = d.user_id;
```

**Usage**: Quickly fetch public key bundles for X3DH initiation.

---

## Migrations

### Initial Setup

```bash
psql -U postgres -d e2ee -f sql_models/seed.sql
```

### Future Migrations

For production, use a migration tool like:

- **[sqlx-cli](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli)** (Rust)
- **[Flyway](https://flywaydb.org/)** (Java/Kotlin)
- **[Liquibase](https://www.liquibase.org/)** (Java/Kotlin)

### Migration Best Practices

1. **Version control** all schema changes
2. **Test migrations** on staging environment first
3. **Use transactions** for atomic migrations
4. **Backup before** applying migrations
5. **Document breaking changes** clearly

---

## Best Practices

### Data Retention

```sql
-- Clean up old used tokens (run daily)
DELETE FROM used_tokens 
WHERE used_at < NOW() - INTERVAL '24 hours';

-- Archive old messages (implement based on policy)
-- Consider moving to cold storage after 90 days
```

### Security

1. **Never store private keys** in the database
2. **Use prepared statements** to prevent SQL injection
3. **Limit exposure** of cryptographic material
4. **Regularly rotate** signed prekeys
5. **Monitor** for anomalous queries

### Performance

1. **Use connection pooling** (SQLx handles this)
2. **Index frequently queried columns**
3. **Partition large tables** (e.g., messages by date)
4. **Archive old data** periodically
5. **Monitor query performance** with `EXPLAIN ANALYZE`

### Backup Strategy

```bash
# Daily full backup
pg_dump -U postgres e2ee > backup_$(date +%Y%m%d).sql

# Continuous WAL archiving for point-in-time recovery
```

---

## Schema Visualization

Use tools like:

- **[pgAdmin](https://www.pgadmin.org/)**: GUI for PostgreSQL
- **[DBeaver](https://dbeaver.io/)**: Universal database tool
- **[dbdiagram.io](https://dbdiagram.io/)**: Online ER diagram creator

---

[← Back to Main Documentation](../README.md)
