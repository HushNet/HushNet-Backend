# 🔒 Security & Cryptography

Comprehensive security documentation for HushNet Backend.

---

## Table of Contents

- [Overview](#overview)
- [Authentication System](#authentication-system)
- [Signal Protocol Implementation](#signal-protocol-implementation)
- [X3DH Key Exchange](#x3dh-key-exchange)
- [Double Ratchet Algorithm](#double-ratchet-algorithm)
- [Cryptographic Primitives](#cryptographic-primitives)
- [Security Properties](#security-properties)
- [Threat Model](#threat-model)
- [Best Practices](#best-practices)
- [Security Audits](#security-audits)

---

## Overview

HushNet Backend implements the **Signal Protocol** to provide end-to-end encrypted (E2EE) messaging with strong security properties including:

- **Forward Secrecy**: Compromise of long-term keys does not compromise past session keys
- **Post-Compromise Security**: Sessions can recover from key compromise
- **Deniability**: Messages are authenticated but not provably from a specific sender
- **Asynchronous Communication**: Recipients can be offline during key exchange

---

## Authentication System

### Ed25519 Signature-Based Authentication

Unlike traditional JWT-based systems, HushNet uses **cryptographic signatures** for authentication:

#### How It Works

1. **Client** generates an Ed25519 key pair (private/public) during device registration
2. For each API request, the **client**:
   - Gets current Unix timestamp
   - Signs the timestamp with the device's private key
   - Sends: `X-Identity-Key`, `X-Signature`, `X-Timestamp` headers

3. **Server** verifies:
   - Timestamp is within 30-second window (anti-replay protection)
   - Signature is valid using the claimed public key
   - Device with that public key exists in database

#### Implementation

```rust
// src/middlewares/auth.rs
pub async fn from_request_parts(
    parts: &mut Parts,
    state: &AppState,
) -> Result<Self, Self::Rejection> {
    // Extract headers
    let ik_b64 = parts.headers.get("X-Identity-Key")?;
    let sig_b64 = parts.headers.get("X-Signature")?;
    let ts = parts.headers.get("X-Timestamp")?;

    // Anti-replay: check timestamp
    let now = chrono::Utc::now().timestamp();
    let ts_i64: i64 = ts.parse()?;
    if (now - ts_i64).abs() > 30 {
        return Err("Expired timestamp");
    }

    // Decode and verify signature
    let sig = Signature::from_bytes(&sig_bytes);
    let vk = VerifyingKey::from_bytes(&vk_arr)?;
    vk.verify(ts.as_bytes(), &sig)?;

    // Fetch device from database
    let device = device_repository::get_device_by_identity_key(
        &state.pool, 
        ik_b64
    ).await?;

    Ok(AuthenticatedDevice(device))
}
```

### Security Benefits

✅ **No token storage**: No JWT secrets to leak or rotate  
✅ **No token expiration management**: Each request is independently verified  
✅ **Strong cryptographic proof**: Based on public-key cryptography  
✅ **Anti-replay protection**: Timestamp window prevents replay attacks  
✅ **Stateless authentication**: No session storage required  

### Security Considerations

⚠️ **Clock synchronization**: Clients must have reasonably accurate clocks  
⚠️ **Private key protection**: Client must securely store device private key  
⚠️ **30-second window**: Tight window balances security vs. clock drift  

---

## Signal Protocol Implementation

The Signal Protocol consists of two main components:

1. **X3DH (Extended Triple Diffie-Hellman)**: Initial key agreement
2. **Double Ratchet**: Ongoing message encryption

---

## X3DH Key Exchange

### Purpose

X3DH enables asynchronous key agreement between two parties who may not be online simultaneously.

### Key Types

| Key Type | Symbol | Purpose | Lifetime |
|----------|--------|---------|----------|
| **Identity Key** | IK | Long-term device identity | Permanent |
| **Signed Prekey** | SPK | Medium-term signed by IK | Rotated periodically |
| **One-Time Prekey** | OPK | Single-use ephemeral keys | One message |
| **Ephemeral Key** | EK | Initiator's temporary key | One handshake |

### Protocol Flow

#### 1. Bob (Recipient) Publishes Keys

Bob uploads to the server:

```json
{
  "identity_pubkey": "IK_B",
  "signed_prekey_pub": "SPK_B", 
  "signed_prekey_sig": "Sig(IK_B, SPK_B)",
  "one_time_prekeys": ["OPK_B_1", "OPK_B_2", ...]
}
```

#### 2. Alice (Initiator) Performs Key Agreement

Alice fetches Bob's public keys and:

1. Generates ephemeral key pair: `(EK_A_priv, EK_A_pub)`
2. Computes Diffie-Hellman exchanges:
   ```
   DH1 = DH(IK_A, SPK_B)
   DH2 = DH(EK_A, IK_B)
   DH3 = DH(EK_A, SPK_B)
   DH4 = DH(EK_A, OPK_B)  [if OPK available]
   ```

3. Derives shared secret:
   ```
   SK = KDF(DH1 || DH2 || DH3 || DH4)
   ```

4. Encrypts initial message with derived key

#### 3. Alice Sends to Bob

```json
{
  "sender_device_id": "alice_device",
  "recipient_device_id": "bob_device",
  "ephemeral_pubkey": "EK_A_pub",
  "sender_prekey_pub": "IK_A",
  "otpk_used": "OPK_B_1",
  "ciphertext": "encrypted_initial_message"
}
```

#### 4. Bob Receives and Computes Shared Secret

Bob performs the same DH computations:

```
DH1 = DH(SPK_B, IK_A)
DH2 = DH(IK_B, EK_A)
DH3 = DH(SPK_B, EK_A)
DH4 = DH(OPK_B, EK_A)  [if OPK was used]

SK = KDF(DH1 || DH2 || DH3 || DH4)
```

Bob can now decrypt the initial message.

### Database Schema

```sql
-- Stores Bob's published keys
CREATE TABLE devices (
  id UUID PRIMARY KEY,
  user_id UUID REFERENCES users(id),
  identity_pubkey TEXT NOT NULL,
  prekey_pubkey TEXT NOT NULL,
  signed_prekey_pub TEXT NOT NULL,
  signed_prekey_sig TEXT NOT NULL,
  one_time_prekeys JSONB NOT NULL  -- Array of OTPKs
);

-- Stores Alice's handshake attempt
CREATE TABLE pending_sessions (
  id UUID PRIMARY KEY,
  sender_device_id UUID REFERENCES devices(id),
  recipient_device_id UUID REFERENCES devices(id),
  ephemeral_pubkey TEXT NOT NULL,
  sender_prekey_pub TEXT NOT NULL,
  otpk_used TEXT NOT NULL,
  ciphertext TEXT NOT NULL,
  state TEXT DEFAULT 'initiated',
  created_at TIMESTAMP DEFAULT NOW()
);
```

### Security Properties

✅ **Mutual Authentication**: Both parties verify each other's identity keys  
✅ **Forward Secrecy**: Ephemeral keys provide forward secrecy  
✅ **Asynchronous**: Recipient doesn't need to be online  
✅ **Deniability**: No non-repudiation signatures  

---

## Double Ratchet Algorithm

### Purpose

After X3DH establishes the initial shared secret, the Double Ratchet provides:

- **Per-message forward secrecy**
- **Post-compromise security** (break-in recovery)
- **Out-of-order message handling**

### How It Works

The Double Ratchet combines:

1. **Symmetric-key ratchet** (hash ratchet): Derives new keys from chain keys
2. **DH ratchet**: Periodically updates root key with new DH exchange

### Message Structure

Each message includes:

```json
{
  "header": {
    "dh_pubkey": "current_ratchet_public_key",
    "pn": 5,  // Previous chain length
    "n": 12   // Message number in current chain
  },
  "ciphertext": "encrypted_message_content"
}
```

### Key Hierarchy

```
Root Key (RK)
    |
    ├─> Chain Key (CK) [Sending]
    |       ├─> Message Key (MK_1)
    |       ├─> Message Key (MK_2)
    |       └─> Message Key (MK_3)
    |
    └─> Chain Key (CK) [Receiving]
            ├─> Message Key (MK_1)
            ├─> Message Key (MK_2)
            └─> Message Key (MK_3)
```

### Ratchet Steps

#### Symmetric-Key Ratchet

```python
# Derive message key from chain key
message_key = HMAC(chain_key, 0x01)

# Advance chain key
chain_key = HMAC(chain_key, 0x02)
```

Each message consumes a unique message key, providing **forward secrecy**.

#### DH Ratchet

Periodically (e.g., every 100 messages or time interval):

```python
# Generate new DH key pair
dh_priv, dh_pub = generate_key_pair()

# Compute shared secret with other party's current DH public key
dh_shared = DH(dh_priv, other_dh_pub)

# Derive new root key and chain key
root_key, chain_key = KDF(root_key, dh_shared)
```

This provides **post-compromise security** (future secrecy).

### Database Schema

```sql
-- Stores session metadata (NOT the keys!)
CREATE TABLE sessions (
  id UUID PRIMARY KEY,
  chat_id UUID REFERENCES chats(id),
  sender_device_id UUID REFERENCES devices(id),
  receiver_device_id UUID REFERENCES devices(id),
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW()
);

-- Messages include ratchet header
CREATE TABLE messages (
  id UUID PRIMARY KEY,
  chat_id UUID REFERENCES chats(id),
  from_device_id UUID REFERENCES devices(id),
  to_device_id UUID REFERENCES devices(id),
  header JSONB NOT NULL,  -- {dh_pubkey, pn, n}
  ciphertext TEXT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT NOW()
);
```

**Important**: The actual root keys, chain keys, and message keys are **NEVER** stored on the server. They exist only in client memory.

### Security Properties

✅ **Forward Secrecy**: Past messages secure even if current keys compromised  
✅ **Post-Compromise Security**: Future messages secure after key recovery  
✅ **Message Authenticity**: AEAD encryption provides authentication  
✅ **Out-of-Order Delivery**: Message numbers allow reordering  

---

## Cryptographic Primitives

### Curve25519

Used for Diffie-Hellman key exchanges in X3DH and Double Ratchet.

**Properties**:
- 128-bit security level
- Fast constant-time implementation
- Widely audited

### Ed25519

Used for digital signatures (authentication, signed prekeys).

**Properties**:
- 128-bit security level
- Small signatures (64 bytes)
- Fast verification
- Deterministic (no need for random nonce)

### HKDF (HMAC-based Key Derivation Function)

Used to derive keys from shared secrets.

```python
derived_keys = HKDF(
    input_key_material=shared_secret,
    salt=optional_salt,
    info=context_string,
    output_length=desired_bytes
)
```

### AEAD (Authenticated Encryption with Associated Data)

Messages are encrypted with AES-256-GCM or ChaCha20-Poly1305:

```python
ciphertext, tag = AEAD_Encrypt(
    key=message_key,
    plaintext=message,
    associated_data=header
)
```

**Benefits**:
- Confidentiality (encryption)
- Authenticity (MAC)
- Associated data binding (header authenticated)

---

## Security Properties

### End-to-End Encryption

✅ **Server cannot read messages**: All content encrypted on client  
✅ **Server cannot decrypt**: No access to encryption keys  
✅ **Metadata protection**: Limited (server knows who talks to whom, when)  

### Forward Secrecy

✅ **Past messages protected**: Compromise of current keys doesn't reveal past  
✅ **Per-message keys**: Each message uses unique ephemeral key  
✅ **DH ratchet**: Regular key rotation  

### Post-Compromise Security

✅ **Self-healing**: System recovers from key compromise  
✅ **Break-in recovery**: After attacker leaves, security restored  
✅ **Fresh DH**: New key agreement establishes new secure channel  

### Authentication

✅ **Mutual authentication**: Both parties verify identity keys  
✅ **Message authenticity**: AEAD provides authenticated encryption  
✅ **Device-level authentication**: Each device has unique identity key  

### Deniability

✅ **Cryptographic deniability**: No provable signatures on messages  
✅ **Anyone can forge**: Symmetric keys allow either party to forge  
⚠️ **Practical limitation**: Server logs may reveal metadata  

---

## Threat Model

### What We Protect Against

#### ✅ Passive Network Attacker

- **Cannot** read message contents
- **Cannot** determine message contents from traffic analysis
- **Can** see metadata (who, when, message sizes)

#### ✅ Active Network Attacker

- **Cannot** inject or modify messages undetected
- **Cannot** impersonate users without device private key
- **Cannot** replay old messages (timestamp validation)

#### ✅ Compromised Server

- **Cannot** read message contents (E2EE)
- **Cannot** decrypt past messages (forward secrecy)
- **Can** deny service
- **Can** collect metadata

#### ✅ Device Compromise (Partial)

- **Past messages protected** (forward secrecy)
- **Future messages can recover** (post-compromise security)
- **Other devices unaffected** (device-level keys)

### What We Don't Protect Against

#### ❌ Malicious Client

If client is compromised, all guarantees fail.

#### ❌ Endpoint Security

We don't protect messages at rest on devices.

#### ❌ Metadata

Server knows:
- Who talks to whom
- When messages are sent
- Message sizes
- Online status

#### ❌ Traffic Analysis

Network observers can perform traffic analysis attacks.

#### ❌ User Compromise

If user's device is stolen with keys accessible, messages can be read.

---

## Best Practices

### Key Management

1. **Rotate signed prekeys regularly** (e.g., weekly)
2. **Replenish one-time prekeys** when low (< 10 remaining)
3. **Securely store private keys** (use OS keychain/secure enclave)
4. **Never transmit private keys** over network
5. **Delete used one-time prekeys** immediately after use

### Session Management

1. **Establish new sessions periodically** (e.g., every 100 messages)
2. **Implement session healing** after delivery failures
3. **Handle out-of-order messages** using message numbers
4. **Store skipped message keys** temporarily for late arrivals
5. **Set limits on skipped keys** (e.g., max 1000) to prevent DOS

### Message Handling

1. **Validate all cryptographic material** before use
2. **Verify signatures** on signed prekeys
3. **Check timestamp windows** (30 seconds for auth, broader for messages)
4. **Implement retry logic** with exponential backoff
5. **Handle decryption failures gracefully**

### Anti-Replay

1. **Enforce timestamp windows** on authentication (30s)
2. **Track used one-time prekeys** to prevent reuse
3. **Implement message deduplication** using logical message IDs
4. **Monitor for suspicious patterns** (rapid repeated requests)

### Operational Security

1. **Use HTTPS/TLS** for all transport (even though E2EE)
2. **Implement rate limiting** to prevent abuse
3. **Log security events** (without sensitive data)
4. **Monitor for anomalies** (e.g., excessive key requests)
5. **Regular security audits** of code and infrastructure

---

## Security Audits

### Recommendations

For production deployment, we recommend:

1. **Professional security audit** of cryptographic implementation
2. **Penetration testing** of infrastructure
3. **Code review** by cryptography experts
4. **Fuzzing** of protocol handling code
5. **Third-party library audits** (verify dependencies)

### Known Limitations

1. **Metadata not protected**: Server sees communication patterns
2. **No sealed sender**: Server knows message sender
3. **No padding**: Message sizes may leak information
4. **No group encryption optimizations**: Each device gets separate message
5. **No backup encryption**: Messages not backed up securely

### Planned Improvements

- [ ] Implement Sealed Sender for metadata protection
- [ ] Add message padding to hide content length
- [ ] Implement Sender Keys for efficient group messaging
- [ ] Add encrypted backup support
- [ ] Implement perfect forward secrecy for authentication

---

## References

### Signal Protocol Documentation

- [Signal Protocol Specification](https://signal.org/docs/)
- [X3DH Specification](https://signal.org/docs/specifications/x3dh/)
- [Double Ratchet Specification](https://signal.org/docs/specifications/doubleratchet/)

### Academic Papers

- Cohn-Gordon, K., Cremers, C., Dowling, B., Garratt, L., & Stebila, D. (2017). "A Formal Security Analysis of the Signal Messaging Protocol"
- Perrin, T., & Marlinspike, M. (2016). "The Double Ratchet Algorithm"

### Cryptographic Libraries

- [ed25519-dalek](https://github.com/dalek-cryptography/ed25519-dalek) - Ed25519 signatures
- [curve25519-dalek](https://github.com/dalek-cryptography/curve25519-dalek) - Curve25519 DH

---

[← Back to Main Documentation](../README.md)
