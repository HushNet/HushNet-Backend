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
