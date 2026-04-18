-- Enable UUID generator
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- =========================
-- Users
-- =========================
CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  username TEXT UNIQUE NOT NULL,
  created_at TIMESTAMP DEFAULT NOW()
);

-- =========================
-- Devices
-- =========================
CREATE TABLE devices (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  identity_pubkey TEXT NOT NULL,
  prekey_pubkey TEXT NOT NULL,
  signed_prekey_pub TEXT NOT NULL,
  signed_prekey_sig TEXT NOT NULL,
  one_time_prekeys JSONB NOT NULL,   -- array of one-time public prekeys
  device_label TEXT,
  push_token TEXT,
  last_seen TIMESTAMP DEFAULT NOW(),
  created_at TIMESTAMP DEFAULT NOW(),
  UNIQUE (user_id, identity_pubkey)
);

-- =========================
-- Chats
-- =========================
CREATE TABLE chats (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  chat_type TEXT CHECK (chat_type IN ('direct','group')) DEFAULT 'direct',

  -- Direct chat participants
  user_a UUID REFERENCES users(id) ON DELETE CASCADE,
  user_b UUID REFERENCES users(id) ON DELETE CASCADE,

  -- Group chat info
  name TEXT,
  owner_id UUID REFERENCES users(id) ON DELETE SET NULL,

  -- Will be linked after messages table is created
  last_message_id UUID,

  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),

  -- For direct chats: both users must be set, and enforce strict ordering
  CONSTRAINT chats_direct_shape
    CHECK (
      chat_type <> 'direct'
      OR (user_a IS NOT NULL AND user_b IS NOT NULL AND user_a < user_b)
    )
);

-- Prevent duplicates (A,B) vs (B,A)
CREATE UNIQUE INDEX uniq_direct_chat
  ON chats (LEAST(user_a, user_b), GREATEST(user_a, user_b))
  WHERE chat_type = 'direct';

-- =========================
-- Chat Members (for group chats)
-- =========================
CREATE TABLE chat_members (
  chat_id UUID REFERENCES chats(id) ON DELETE CASCADE,
  user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  role TEXT DEFAULT 'member',
  joined_at TIMESTAMP DEFAULT NOW(),
  PRIMARY KEY (chat_id, user_id)
);

-- =========================
-- Messages
-- =========================
CREATE TABLE messages (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

  -- Logical message id (shared by all device fanouts)
  logical_msg_id TEXT NOT NULL,

  chat_id UUID REFERENCES chats(id) ON DELETE CASCADE,

  from_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  from_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,

  to_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  to_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,

  header JSONB NOT NULL,       -- Double Ratchet header (DH pubkey, counters, etc.)
  ciphertext TEXT NOT NULL,    -- base64(nonce || cipher || mac)

  delivered_at TIMESTAMPTZ,
  read_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Add FK once messages table exists
ALTER TABLE chats
  ADD CONSTRAINT fk_chats_last_message
  FOREIGN KEY (last_message_id)
  REFERENCES messages(id)
  ON DELETE SET NULL;

-- Indexes
CREATE INDEX idx_messages_todevice ON messages(to_device_id, created_at);
CREATE INDEX idx_messages_chatid   ON messages(chat_id, created_at);
CREATE INDEX idx_messages_logical  ON messages(logical_msg_id);

-- =========================
-- Sessions (metadata only, no secret keys)
-- =========================
CREATE TABLE sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  chat_id UUID REFERENCES chats(id) ON DELETE CASCADE,
  sender_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  receiver_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  UNIQUE (sender_device_id, receiver_device_id)
);

-- =========================
-- Used Tokens (anti-replay or enrollment)
-- =========================
CREATE TABLE used_tokens (
  token TEXT PRIMARY KEY,
  used_at TIMESTAMP DEFAULT NOW()
);

-- =========================
-- Pending Sessions (X3DH handshake initialization)
-- =========================
CREATE TABLE pending_sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  sender_device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  recipient_device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  ephemeral_pubkey TEXT NOT NULL,     -- Ephemeral key (EK_A pub)
  sender_prekey_pub TEXT NOT NULL,    -- IK/SPK_A used by sender
  otpk_used TEXT NOT NULL,            -- Whether a one-time prekey was consumed
  ciphertext TEXT NOT NULL,           -- Encrypted payload (init message)
  created_at TIMESTAMP DEFAULT NOW(),
  state TEXT DEFAULT 'initiated' CHECK (state IN ('initiated','responded','completed'))
);

CREATE INDEX idx_pending_sessions_recipient ON pending_sessions(recipient_device_id, created_at);

-- =========================
-- Convenience View: Public device list
-- =========================
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


ALTER TABLE messages
ALTER COLUMN created_at TYPE timestamptz
USING created_at AT TIME ZONE 'Europe/Paris';

ALTER TABLE messages
ALTER COLUMN delivered_at TYPE timestamptz
USING delivered_at AT TIME ZONE 'Europe/Paris';

ALTER TABLE messages
ALTER COLUMN read_at TYPE timestamptz
USING read_at AT TIME ZONE 'Europe/Paris';

ALTER TABLE messages
ALTER COLUMN created_at SET DEFAULT (NOW() AT TIME ZONE 'UTC');
ALTER TABLE messages
ALTER COLUMN delivered_at SET DEFAULT NULL;
ALTER TABLE messages
ALTER COLUMN read_at SET DEFAULT NULL;

-- ================
-- Messages channel
-- ================
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

DROP TRIGGER IF EXISTS messages_notify_trigger ON messages;
CREATE TRIGGER messages_notify_trigger
AFTER INSERT ON messages
FOR EACH ROW
EXECUTE FUNCTION notify_new_message();

-- ================
-- Sessions channel
-- ================
CREATE OR REPLACE FUNCTION notify_new_session() RETURNS trigger AS $$
DECLARE
  sender_user UUID;
  receiver_user UUID;
BEGIN
  -- Retrieve user IDs for sender and receiver devices
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

-- ================
-- Devices channel
-- ================
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

-- =========================
-- Pending Sessions channel
-- =========================
CREATE OR REPLACE FUNCTION notify_new_pending_session() RETURNS trigger AS $$
DECLARE
  recipient_user UUID;
BEGIN
  SELECT u.id INTO recipient_user
  FROM users u
  JOIN devices d ON d.user_id = u.id
  WHERE d.id = NEW.recipient_device_id;

  RAISE NOTICE 'Trigger fired: sender_device=%, recipient_device=%, found_user=%',
    NEW.sender_device_id, NEW.recipient_device_id, recipient_user;

  IF recipient_user IS NOT NULL THEN
    PERFORM pg_notify(
      'pending_sessions_channel',
      json_build_object(
        'type', 'pending_session',
        'user_id', recipient_user,
        'recipient_device_id', NEW.recipient_device_id,
        'sender_device_id', NEW.sender_device_id,
        'pending_session_id', NEW.id,
        'created_at', NEW.created_at
      )::text
    );
    RAISE NOTICE 'Notification sent to user_id=%', recipient_user;
  ELSE
    RAISE NOTICE 'No recipient_user found for device_id=%', NEW.recipient_device_id;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;


DROP TRIGGER IF EXISTS pending_sessions_notify_trigger ON pending_sessions;
CREATE TRIGGER pending_sessions_notify_trigger
AFTER INSERT ON pending_sessions
FOR EACH ROW
EXECUTE FUNCTION notify_new_pending_session();

-- =============================================================================
-- Migration: inter-node federation support
--
-- Run this after sql_models/seed.sql. Every change here is purely additive:
-- no existing column is dropped or renamed, no existing constraint is altered.
--
-- The three new tables (federation_nodes, used_node_nonces, federation_outbox)
-- and the two new columns on users (home_node_id, federated_address) are the
-- only schema deltas required to support cross-node message routing.
-- =============================================================================

-- -----------------------------------------------------------------------------
-- Peer node registry
--
-- One row per known peer. Rows are created lazily: the first time this node
-- receives an S2S request from an unknown peer, it fetches that peer's record
-- from the central registry and inserts it here.
--
-- public_key_b64 is the Ed25519 verifying key used to authenticate every
-- inbound S2S request from that peer.  It must match what the peer registered
-- at the central registry (registry.hushnet.net).
--
-- is_blocked allows an operator to stop accepting traffic from a specific peer
-- without removing the row (which would just re-create it on the next contact).
-- -----------------------------------------------------------------------------
CREATE TABLE federation_nodes (
  id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  node_id        TEXT        UNIQUE NOT NULL,   -- "node-a.hushnet.net"
  api_url        TEXT        NOT NULL,           -- "https://node-a.hushnet.net/api"
  public_key_b64 TEXT        NOT NULL,           -- Ed25519 verifying key, base64
  last_seen      TIMESTAMPTZ,
  is_blocked     BOOLEAN     NOT NULL DEFAULT FALSE,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- -----------------------------------------------------------------------------
-- Anti-replay nonce store
--
-- Every accepted S2S request carries a random 16-byte nonce (base64-encoded).
-- The pair (node_id, nonce) is stored here immediately after signature
-- verification to prevent exact-replay attacks within the timestamp acceptance
-- window (currently 60 s).
--
-- Rows older than 5 minutes can be safely deleted; the outbox worker runs a
-- periodic purge via:
--   DELETE FROM used_node_nonces WHERE used_at < NOW() - INTERVAL '5 minutes'
-- -----------------------------------------------------------------------------
CREATE TABLE used_node_nonces (
  nonce    TEXT        NOT NULL,
  node_id  TEXT        NOT NULL,
  used_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (nonce, node_id)
);

-- Supports cheap TTL-based cleanup without a sequential scan.
CREATE INDEX idx_used_node_nonces_used_at ON used_node_nonces (used_at);

-- -----------------------------------------------------------------------------
-- Outbound delivery queue (federation outbox)
--
-- Every logical message addressed to a remote node is written here before any
-- network call is made.  The outbox worker reads pending entries, attempts
-- delivery to the target node, and transitions entries to 'delivered' or
-- 'failed'.
--
-- payload is the verbatim JSON body of the POST /s2s/messages request that
-- will be sent to the target node. Storing it here means retries require no
-- additional DB reads to reconstruct the request.
--
-- Backoff schedule implemented by the worker (seconds):
--   10, 20, 40, 80, 160, 320, 640, 1280, 2560, 3600 (cap)
-- After 10 failed attempts the entry is marked 'failed' and abandoned.
-- -----------------------------------------------------------------------------
CREATE TABLE federation_outbox (
  id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  target_node_id TEXT        NOT NULL,           -- destination node_id
  logical_msg_id TEXT        NOT NULL,
  payload        JSONB       NOT NULL,
  attempt_count  INT         NOT NULL DEFAULT 0,
  last_attempt   TIMESTAMPTZ,
  next_attempt   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  status         TEXT        NOT NULL DEFAULT 'pending'
                   CHECK (status IN ('pending', 'delivered', 'failed')),
  created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Partial index: only pending entries participate in the outbox work loop.
CREATE INDEX idx_federation_outbox_work
  ON federation_outbox (target_node_id, next_attempt)
  WHERE status = 'pending';

-- -----------------------------------------------------------------------------
-- Users: federation columns
--
-- home_node_id NULL  → the user is local to this node; their devices are
--                       authoritative here and they can log in normally.
--
-- home_node_id SET   → shadow record for a remote user whose real account lives
--                       on another node. Created automatically the first time
--                       this node receives a message from that user. Shadow
--                       users cannot register devices or log in here; they
--                       exist only to satisfy foreign-key constraints on the
--                       messages and pending_sessions tables.
--
-- federated_address is globally unique across all nodes:
--   "alice@node-a.hushnet.net"
-- It is the canonical identifier for cross-node addressing. Username alone is
-- not globally unique since each node has its own namespace.
-- -----------------------------------------------------------------------------
ALTER TABLE users
  ADD COLUMN home_node_id      UUID REFERENCES federation_nodes(id) ON DELETE SET NULL,
  ADD COLUMN federated_address TEXT UNIQUE;

-- After running this migration, populate federated_address for every existing
-- local user by substituting the actual NODE_HOST value:
--
--   UPDATE users
--   SET federated_address = username || '@<NODE_HOST>'
--   WHERE home_node_id IS NULL AND federated_address IS NULL;
--
-- This can be run as a separate step; the column is nullable so existing rows
-- are not broken before the backfill runs.

-- -----------------------------------------------------------------------------
-- Messages: deduplication constraint
--
-- A given (logical_msg_id, to_device_id) pair must appear at most once in the
-- messages table. This is already implied by correct client behavior (one
-- logical message fan-out produces exactly one row per recipient device), but
-- the unique constraint makes idempotent S2S delivery safe: the receiving node
-- can INSERT ... ON CONFLICT DO NOTHING and check rows_affected to distinguish
-- a fresh delivery from a duplicate.
--
-- If existing data violates this constraint (which it should not under correct
-- operation), the migration will fail and duplicates must be resolved first.
-- -----------------------------------------------------------------------------
ALTER TABLE messages
  ADD CONSTRAINT uniq_message_per_device UNIQUE (logical_msg_id, to_device_id);
