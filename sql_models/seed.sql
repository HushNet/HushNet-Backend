CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  username TEXT UNIQUE NOT NULL,
  created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE messages (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  from_user UUID REFERENCES users(id),
  to_user UUID REFERENCES users(id),
  ciphertext TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT NOW(),
  delivered BOOLEAN DEFAULT FALSE
);

CREATE TABLE devices (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  identity_pubkey TEXT NOT NULL, 
  device_label TEXT,
  push_token TEXT,
  last_seen TIMESTAMP DEFAULT NOW(),
  created_at TIMESTAMP DEFAULT NOW(),
  UNIQUE(user_id, identity_pubkey)
);

CREATE TABLE signed_prekeys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  key TEXT NOT NULL,                     -- SPK publique
  signature TEXT NOT NULL,               -- sig(SPK, IK_priv)
  created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE one_time_prekeys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  device_id UUID REFERENCES devices(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  used BOOLEAN DEFAULT FALSE,
  created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE chats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_a_id UUID REFERENCES users(id) ON DELETE CASCADE,
    user_b_id UUID REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(user_a_id, user_b_id)
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
    ratchet_priv BYTEA,
    last_remote_pub BYTEA,

    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),

    UNIQUE(sender_device_id, receiver_device_id)
);
