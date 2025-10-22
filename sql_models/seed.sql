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

  delivered_at TIMESTAMP,
  read_at TIMESTAMP,
  created_at TIMESTAMP DEFAULT NOW()
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
