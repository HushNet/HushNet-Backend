# 🌐 API Reference

Complete API documentation for HushNet Backend.

---

## Table of Contents

- [Authentication](#authentication)
- [Root Endpoints](#root-endpoints)
- [User Endpoints](#user-endpoints)
- [Device Endpoints](#device-endpoints)
- [Session Endpoints](#session-endpoints)
- [Chat Endpoints](#chat-endpoints)
- [Message Endpoints](#message-endpoints)
- [WebSocket Endpoints](#websocket-endpoints)
- [Error Responses](#error-responses)

---

## Authentication

HushNet uses **Ed25519 signature-based authentication** instead of JWT. Each authenticated request must include the following headers:

### Required Headers

| Header | Description | Format |
|--------|-------------|--------|
| `X-Identity-Key` | Base64-encoded Ed25519 public key (32 bytes) | `base64(public_key)` |
| `X-Signature` | Base64-encoded signature (64 bytes) | `base64(signature)` |
| `X-Timestamp` | Unix timestamp (seconds) | `1698765432` |

### Authentication Flow

1. **Client** generates a current Unix timestamp
2. **Client** signs the timestamp with their Ed25519 private key
3. **Client** sends request with headers:
   - `X-Identity-Key`: Device's identity public key
   - `X-Signature`: Signature of the timestamp
   - `X-Timestamp`: The timestamp that was signed
4. **Server** verifies:
   - Timestamp is within 30 seconds window (anti-replay)
   - Signature is valid for the given public key
   - Device exists with that identity key

### Example (JavaScript)

```javascript
import { sign } from '@noble/ed25519';

const timestamp = Math.floor(Date.now() / 1000).toString();
const signature = await sign(
  new TextEncoder().encode(timestamp),
  privateKey
);

const headers = {
  'X-Identity-Key': base64Encode(publicKey),
  'X-Signature': base64Encode(signature),
  'X-Timestamp': timestamp,
  'Content-Type': 'application/json'
};
```

### Authentication Errors

| Status | Error | Description |
|--------|-------|-------------|
| `401` | Missing X-Identity-Key | Header not provided |
| `401` | Missing X-Signature | Header not provided |
| `401` | Missing X-Timestamp | Header not provided |
| `401` | Expired timestamp | Timestamp outside 30s window |
| `401` | Signature mismatch | Invalid signature |
| `401` | Unknown device | Device not registered |
| `400` | Bad signature b64 | Invalid base64 encoding |
| `400` | Signature must be 64 bytes | Wrong signature length |
| `400` | Bad pubkey b64 | Invalid base64 encoding |
| `400` | Bad pubkey length | Public key not 32 bytes |
| `400` | Bad pubkey | Invalid Ed25519 public key |

---

## Root Endpoints

### GET `/`

Health check endpoint.

**Authentication**: Not required

**Response**: `200 OK`

```json
{
  "message": "Hello from HushNet Backend",
  "version": "0.1.0",
  "status": "healthy"
}
```

---

## User Endpoints

### GET `/users`

List all users.

**Authentication**: Not required

**Response**: `200 OK`

```json
[
  {
    "id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
    "username": "alice",
    "created_at": "2025-11-02T10:30:00Z"
  },
  {
    "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "username": "bob",
    "created_at": "2025-11-02T11:00:00Z"
  }
]
```

---

### POST `/users`

Create a new user.

**Authentication**: Not required

**Request Body**:

```json
{
  "username": "alice"
}
```

**Response**: `201 Created`

```json
{
  "id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "username": "alice",
  "created_at": "2025-11-02T10:30:00Z"
}
```

**Errors**:

- `400 Bad Request`: Invalid username
- `409 Conflict`: Username already exists

---

### GET `/users/:id`

Get user by ID.

**Authentication**: Required

**Parameters**:

- `id` (UUID): User ID

**Response**: `200 OK`

```json
{
  "id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "username": "alice",
  "created_at": "2025-11-02T10:30:00Z"
}
```

**Errors**:

- `401 Unauthorized`: Invalid authentication
- `404 Not Found`: User does not exist

---

### POST `/login`

Authenticate a user (returns user info for verification).

**Authentication**: Not required

**Request Body**:

```json
{
  "username": "alice",
  "identity_pubkey": "base64_encoded_public_key"
}
```

**Response**: `200 OK`

```json
{
  "user_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "device_id": "d1e2f3a4-b5c6-7890-abcd-ef1234567890",
  "username": "alice"
}
```

**Errors**:

- `401 Unauthorized`: Invalid credentials
- `404 Not Found`: User or device not found

---

## Device Endpoints

### GET `/devices/:user_id`

List all devices for a user.

**Authentication**: Required

**Parameters**:

- `user_id` (UUID): User ID

**Response**: `200 OK`

```json
[
  {
    "id": "d1e2f3a4-b5c6-7890-abcd-ef1234567890",
    "user_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
    "identity_pubkey": "base64_encoded_key",
    "prekey_pubkey": "base64_encoded_key",
    "signed_prekey_pub": "base64_encoded_key",
    "signed_prekey_sig": "base64_encoded_sig",
    "one_time_prekeys": ["key1", "key2", "key3"],
    "device_label": "iPhone 15 Pro",
    "push_token": "apns_token_here",
    "last_seen": "2025-11-02T14:30:00Z",
    "created_at": "2025-11-02T10:30:00Z"
  }
]
```

---

### POST `/devices`

Register a new device.

**Authentication**: Required

**Request Body**:

```json
{
  "user_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "identity_pubkey": "base64_encoded_identity_key",
  "prekey_pubkey": "base64_encoded_prekey",
  "signed_prekey_pub": "base64_encoded_signed_prekey_pub",
  "signed_prekey_sig": "base64_encoded_signed_prekey_signature",
  "one_time_prekeys": [
    "base64_encoded_otpk_1",
    "base64_encoded_otpk_2",
    "base64_encoded_otpk_3"
  ],
  "device_label": "iPhone 15 Pro",
  "push_token": "apns_or_fcm_token"
}
```

**Response**: `201 Created`

```json
{
  "id": "d1e2f3a4-b5c6-7890-abcd-ef1234567890",
  "user_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "identity_pubkey": "base64_encoded_identity_key",
  "device_label": "iPhone 15 Pro",
  "created_at": "2025-11-02T10:30:00Z"
}
```

**Errors**:

- `400 Bad Request`: Invalid device data
- `401 Unauthorized`: Invalid authentication
- `409 Conflict`: Device already exists

---

### GET `/devices/keys/:username`

Get public keys for all devices of a user (for key exchange).

**Authentication**: Required

**Parameters**:

- `username` (string): Username

**Response**: `200 OK`

```json
{
  "user_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "username": "alice",
  "devices": [
    {
      "device_id": "d1e2f3a4-b5c6-7890-abcd-ef1234567890",
      "identity_pubkey": "base64_encoded_key",
      "signed_prekey_pub": "base64_encoded_key",
      "signed_prekey_sig": "base64_encoded_sig",
      "one_time_prekey": "base64_encoded_otpk"
    }
  ]
}
```

**Note**: One-time prekeys are consumed after retrieval.

---

## Session Endpoints

### POST `/sessions`

Initiate an X3DH session with another device.

**Authentication**: Required

**Request Body**:

```json
{
  "sender_device_id": "your_device_uuid",
  "recipient_device_id": "recipient_device_uuid",
  "ephemeral_pubkey": "base64_encoded_ephemeral_key",
  "sender_prekey_pub": "base64_encoded_sender_prekey",
  "otpk_used": "base64_encoded_one_time_prekey_used",
  "ciphertext": "base64_encoded_initial_message"
}
```

**Response**: `201 Created`

```json
{
  "session_id": "session-uuid",
  "status": "pending",
  "created_at": "2025-11-02T10:30:00Z"
}
```

---

### GET `/sessions/pending`

Get pending session requests for the authenticated device.

**Authentication**: Required

**Query Parameters**:

- `device_id` (UUID): Device ID

**Response**: `200 OK`

```json
[
  {
    "id": "session-uuid",
    "sender_device_id": "sender-device-uuid",
    "recipient_device_id": "your-device-uuid",
    "ephemeral_pubkey": "base64_encoded_key",
    "sender_prekey_pub": "base64_encoded_key",
    "otpk_used": "base64_encoded_key",
    "ciphertext": "base64_encoded_message",
    "state": "initiated",
    "created_at": "2025-11-02T10:30:00Z"
  }
]
```

---

### POST `/sessions/confirm`

Confirm and complete a session.

**Authentication**: Required

**Request Body**:

```json
{
  "pending_session_id": "session-uuid",
  "chat_id": "chat-uuid"
}
```

**Response**: `200 OK`

```json
{
  "session_id": "confirmed-session-uuid",
  "status": "completed",
  "chat_id": "chat-uuid"
}
```

---

## Chat Endpoints

### GET `/chats`

Get all chats for the authenticated user.

**Authentication**: Required

**Query Parameters**:

- `user_id` (UUID): User ID

**Response**: `200 OK`

```json
[
  {
    "id": "chat-uuid",
    "chat_type": "direct",
    "user_a": "user-uuid-1",
    "user_b": "user-uuid-2",
    "name": null,
    "last_message_id": "message-uuid",
    "created_at": "2025-11-02T10:00:00Z",
    "updated_at": "2025-11-02T14:30:00Z"
  },
  {
    "id": "chat-uuid-2",
    "chat_type": "group",
    "name": "Project Team",
    "owner_id": "user-uuid-1",
    "last_message_id": "message-uuid-2",
    "created_at": "2025-11-01T09:00:00Z",
    "updated_at": "2025-11-02T15:00:00Z"
  }
]
```

---

### GET `/chats/:chat_id/devices`

Get all device IDs participating in a chat.

**Authentication**: Required

**Parameters**:

- `chat_id` (UUID): Chat ID

**Response**: `200 OK`

```json
{
  "chat_id": "chat-uuid",
  "devices": [
    "device-uuid-1",
    "device-uuid-2",
    "device-uuid-3"
  ]
}
```

---

## Message Endpoints

### POST `/messages`

Send an encrypted message.

**Authentication**: Required

**Request Body**:

```json
{
  "chat_id": "chat-uuid",
  "to_device_id": "recipient-device-uuid",
  "header": {
    "dh_pubkey": "base64_encoded_ratchet_key",
    "pn": 0,
    "n": 1
  },
  "ciphertext": "base64_encoded_encrypted_content"
}
```

**Response**: `201 Created`

```json
{
  "message_id": "message-uuid",
  "logical_msg_id": "logical-uuid",
  "created_at": "2025-11-02T14:30:00Z"
}
```

---

### GET `/messages/pending/:device_id`

Get pending messages for a device.

**Authentication**: Required

**Parameters**:

- `device_id` (UUID): Device ID

**Response**: `200 OK`

```json
[
  {
    "id": "message-uuid",
    "logical_msg_id": "logical-uuid",
    "chat_id": "chat-uuid",
    "from_user_id": "sender-user-uuid",
    "from_device_id": "sender-device-uuid",
    "to_user_id": "your-user-uuid",
    "to_device_id": "your-device-uuid",
    "header": {
      "dh_pubkey": "base64_key",
      "pn": 0,
      "n": 1
    },
    "ciphertext": "base64_encrypted_content",
    "delivered_at": null,
    "read_at": null,
    "created_at": "2025-11-02T14:30:00Z"
  }
]
```

---

## WebSocket Endpoints

### WS `/ws?user_id=<uuid>`

Establish a WebSocket connection for real-time events.

**Authentication**: Query parameter `user_id` required

**Connection**:

```javascript
const ws = new WebSocket('ws://127.0.0.1:8080/ws?user_id=<user-uuid>');
```

### Event Types

#### 1. New Message

```json
{
  "type": "message",
  "chat_id": "chat-uuid",
  "user_id": "recipient-user-uuid"
}
```

**Action**: Fetch pending messages for the chat.

#### 2. New Session

```json
{
  "type": "session",
  "user_id": "affected-user-uuid",
  "sender_device_id": "sender-device-uuid",
  "receiver_device_id": "receiver-device-uuid"
}
```

**Action**: Fetch pending sessions.

#### 3. Device Update

```json
{
  "type": "device",
  "user_id": "user-uuid"
}
```

**Action**: Refresh device list for the user.

---

## Error Responses

### Standard Error Format

```json
{
  "error": "Error message description",
  "code": "ERROR_CODE",
  "status": 400
}
```

### Common HTTP Status Codes

| Status Code | Description |
|-------------|-------------|
| `200 OK` | Request successful |
| `201 Created` | Resource created successfully |
| `400 Bad Request` | Invalid request data |
| `401 Unauthorized` | Authentication failed |
| `403 Forbidden` | Access denied |
| `404 Not Found` | Resource not found |
| `409 Conflict` | Resource already exists |
| `422 Unprocessable Entity` | Validation failed |
| `500 Internal Server Error` | Server error |

---

## Rate Limiting

Currently, there is no rate limiting implemented. For production deployments, consider implementing rate limiting at the application or infrastructure level.

### Recommended Limits

- **Authentication requests**: 5 per minute per IP
- **Message sending**: 60 per minute per device
- **WebSocket connections**: 5 concurrent per user
- **API requests**: 100 per minute per device

---

## Best Practices

1. **Always verify signatures** on the client side before sending
2. **Handle timestamp synchronization** - ensure client clock is accurate
3. **Implement exponential backoff** for failed requests
4. **Cache device keys** to reduce API calls
5. **Use WebSocket for real-time updates** instead of polling
6. **Validate all cryptographic materials** before use
7. **Implement proper error handling** for all API calls

---

[← Back to Main Documentation](../README.md)
