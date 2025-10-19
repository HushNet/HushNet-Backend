CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  username TEXT UNIQUE NOT NULL,
  created_at TIMESTAMP DEFAULT NOW()
);

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
  UNIQUE(user_id, identity_pubkey)
);

CREATE TABLE chats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_a_id UUID REFERENCES users(id) ON DELETE CASCADE,
    user_b_id UUID REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(user_a_id, user_b_id)
);


CREATE TABLE messages (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  chat_id UUID REFERENCES chats(id) ON DELETE CASCADE,
  from_device_id UUID REFERENCES devices(id) ON DELETE SET NULL,
  plaintext_hash TEXT,                -- SHA256 du plaintext avant chiffrement
  created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE message_deliveries (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  message_id UUID REFERENCES messages(id) ON DELETE CASCADE,
  to_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  ciphertext TEXT NOT NULL,
  delivered BOOLEAN DEFAULT FALSE,
  created_at TIMESTAMP DEFAULT NOW()
);


CREATE TABLE sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  chat_id UUID REFERENCES chats(id) ON DELETE CASCADE,
  sender_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  receiver_device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  root_key BYTEA NOT NULL,
  send_chain_key BYTEA,
  recv_chain_key BYTEA,
  send_counter INTEGER DEFAULT 0,
  recv_counter INTEGER DEFAULT 0,
  ratchet_pub BYTEA,
  last_remote_pub BYTEA,
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  UNIQUE(sender_device_id, receiver_device_id)
);

CREATE TABLE used_tokens (
  token TEXT PRIMARY KEY,
  used_at TIMESTAMP DEFAULT NOW()
);


CREATE TABLE pending_sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  sender_device_id UUID NOT NULL REFERENCES devices(id),
  recipient_device_id UUID NOT NULL REFERENCES devices(id),
  ephemeral_pubkey TEXT NOT NULL,
  sender_prekey_pub TEXT NOT NULL,
  otpk_used TEXT NOT NULL,
  ciphertext TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT NOW(),
  state TEXT DEFAULT 'initiated' CHECK (state IN ('initiated', 'responded', 'completed'))
);

ALTER TABLE chats
ADD CONSTRAINT chats_unique_pair CHECK (user_a_id < user_b_id);


CREATE VIEW user_devices_view AS
SELECT 
  u.username,
  d.id AS device_id,
  d.identity_pubkey,
  d.signed_prekey_pub,
  d.signed_prekey_sig,
  (d.one_time_prekeys -> 0) AS one_time_prekey_pub
FROM users u
JOIN devices d ON u.id = d.user_id;