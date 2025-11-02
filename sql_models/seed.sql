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
  SELECT user_id INTO recipient_user
  FROM devices
  WHERE id = NEW.recipient_device_id;

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
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS pending_sessions_notify_trigger ON pending_sessions;
CREATE TRIGGER pending_sessions_notify_trigger
AFTER INSERT ON pending_sessions
FOR EACH ROW
EXECUTE FUNCTION notify_new_pending_session();
