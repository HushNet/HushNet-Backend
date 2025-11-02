# ⚡ WebSocket & Real-time Communication

Documentation for HushNet's real-time messaging system.

---

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [WebSocket Connection](#websocket-connection)
- [Event Types](#event-types)
- [PostgreSQL LISTEN/NOTIFY](#postgresql-listennotify)
- [Client Implementation](#client-implementation)
- [Error Handling](#error-handling)
- [Performance Considerations](#performance-considerations)

---

## Overview

HushNet implements real-time communication using:

- **WebSockets** for client connections
- **PostgreSQL LISTEN/NOTIFY** for event broadcasting
- **Tokio broadcast channels** for in-memory event distribution

This approach provides:

✅ Instant message delivery notifications  
✅ Real-time session establishment alerts  
✅ Device update notifications  
✅ Scalable event distribution  
✅ Database-driven reliability  

---

## Architecture

```
┌─────────────┐
│  PostgreSQL │
│   Database  │
└──────┬──────┘
       │ INSERT/UPDATE
       ↓ (Triggers)
┌──────────────┐
│   NOTIFY     │ messages_channel
│   NOTIFY     │ sessions_channel
│   NOTIFY     │ devices_channel
└──────┬───────┘
       │
       ↓ LISTEN (polling)
┌──────────────────┐
│ PG Listener Task │ (Tokio task)
└──────┬───────────┘
       │
       ↓ Broadcast
┌──────────────────────┐
│  Tokio Broadcast     │ (in-memory channel)
│  Channel<Event>      │
└──────┬───────────────┘
       │
       ↓ Subscribe
┌──────────────────────┐
│ WebSocket Handlers   │ (per connection)
│   - Filter by user   │
│   - Send to client   │
└──────────────────────┘
       │
       ↓ WebSocket
┌──────────────────────┐
│   Connected Clients  │
└──────────────────────┘
```

### Flow

1. **Database event** (INSERT message, session, device update)
2. **Trigger executes** → `NOTIFY` on channel
3. **PG Listener** receives notification
4. **Broadcast** to all WebSocket handlers
5. **Filter** by user_id (each handler knows its user)
6. **Send** to WebSocket client

---

## WebSocket Connection

### Endpoint

```
ws://127.0.0.1:8080/ws?user_id=<user-uuid>
```

### Authentication

User ID must be provided in query parameter. Future versions may use token-based auth for WebSocket connections.

### Connection Lifecycle

```javascript
const ws = new WebSocket('ws://127.0.0.1:8080/ws?user_id=<uuid>');

// Connection opened
ws.onopen = () => {
  console.log('WebSocket connected');
};

// Receive events
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  handleRealtimeEvent(data);
};

// Connection closed
ws.onclose = (event) => {
  console.log('WebSocket closed:', event.code, event.reason);
  // Implement reconnection logic
};

// Error handling
ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};
```

### Reconnection Strategy

```javascript
class ReconnectingWebSocket {
  constructor(url, userId) {
    this.url = url;
    this.userId = userId;
    this.reconnectDelay = 1000; // Start with 1 second
    this.maxReconnectDelay = 30000; // Max 30 seconds
    this.connect();
  }

  connect() {
    this.ws = new WebSocket(`${this.url}?user_id=${this.userId}`);
    
    this.ws.onopen = () => {
      console.log('Connected');
      this.reconnectDelay = 1000; // Reset delay on successful connection
    };

    this.ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      this.handleEvent(data);
    };

    this.ws.onclose = () => {
      console.log(`Reconnecting in ${this.reconnectDelay}ms...`);
      setTimeout(() => this.connect(), this.reconnectDelay);
      
      // Exponential backoff
      this.reconnectDelay = Math.min(
        this.reconnectDelay * 2, 
        this.maxReconnectDelay
      );
    };
  }

  handleEvent(event) {
    // Your event handling logic
  }

  close() {
    this.ws.close();
  }
}
```

---

## Event Types

### 1. New Message Event

Sent when a new message is inserted for the user.

```json
{
  "type": "message",
  "chat_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "user_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
}
```

**Action**: Fetch pending messages for the specified chat.

```javascript
if (event.type === 'message') {
  fetchPendingMessages(event.chat_id);
}
```

### 2. New Session Event

Sent when a new Double Ratchet session is established.

```json
{
  "type": "session",
  "user_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "sender_device_id": "sender-device-uuid",
  "receiver_device_id": "receiver-device-uuid"
}
```

**Action**: Fetch pending sessions and establish session state.

```javascript
if (event.type === 'session') {
  fetchPendingSessions();
  // Initialize Double Ratchet state
}
```

### 3. Device Update Event

Sent when a device is registered or keys are updated.

```json
{
  "type": "device",
  "user_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
}
```

**Action**: Refresh device list and public keys.

```javascript
if (event.type === 'device') {
  refreshDeviceList(event.user_id);
  // May need to re-fetch prekeys
}
```

---

## PostgreSQL LISTEN/NOTIFY

### Channels

HushNet uses three PostgreSQL notification channels:

- `messages_channel`: New message notifications
- `sessions_channel`: New session notifications
- `devices_channel`: Device update notifications

### Listener Implementation

```rust
// src/realtime/listener.rs
pub async fn start_pg_listeners(
    pool: PgPool,
    tx: broadcast::Sender<RealtimeEvent>,
) {
    tokio::spawn(async move {
        let mut listener = PgListener::connect_with(&pool)
            .await
            .expect("Failed to create listener");

        listener
            .listen_all(vec![
                "messages_channel",
                "sessions_channel",
                "devices_channel",
            ])
            .await
            .expect("Failed to listen to channels");

        loop {
            while let Ok(notification) = listener.try_recv().await {
                if let Some(notif) = notification {
                    let payload: RealtimeEvent = 
                        serde_json::from_str(notif.payload()).ok()?;
                    
                    // Broadcast to all WebSocket handlers
                    let _ = tx.send(payload);
                }
            }
        }
    });
}
```

### Trigger Functions

```sql
-- Messages trigger
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

---

## Client Implementation

### JavaScript/TypeScript Example

```typescript
class HushNetClient {
  private ws: WebSocket | null = null;
  private userId: string;
  private reconnectAttempts = 0;

  constructor(userId: string) {
    this.userId = userId;
    this.connect();
  }

  private connect() {
    const wsUrl = `ws://localhost:8080/ws?user_id=${this.userId}`;
    this.ws = new WebSocket(wsUrl);

    this.ws.onopen = () => {
      console.log('✅ WebSocket connected');
      this.reconnectAttempts = 0;
    };

    this.ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      this.handleEvent(data);
    };

    this.ws.onclose = () => {
      console.log('❌ WebSocket disconnected');
      this.reconnect();
    };

    this.ws.onerror = (error) => {
      console.error('WebSocket error:', error);
    };
  }

  private handleEvent(event: RealtimeEvent) {
    switch (event.type) {
      case 'message':
        this.onNewMessage(event);
        break;
      case 'session':
        this.onNewSession(event);
        break;
      case 'device':
        this.onDeviceUpdate(event);
        break;
    }
  }

  private async onNewMessage(event: MessageEvent) {
    console.log('📨 New message in chat:', event.chat_id);
    
    // Fetch pending messages
    const messages = await fetch(
      `/messages/pending/${this.currentDeviceId}`
    ).then(r => r.json());
    
    // Decrypt and display messages
    for (const msg of messages) {
      const decrypted = await this.decryptMessage(msg);
      this.displayMessage(decrypted);
    }
  }

  private async onNewSession(event: SessionEvent) {
    console.log('🔐 New session established');
    
    // Fetch pending sessions
    const sessions = await fetch(
      `/sessions/pending?device_id=${this.currentDeviceId}`
    ).then(r => r.json());
    
    // Process X3DH handshakes
    for (const session of sessions) {
      await this.completeX3DH(session);
    }
  }

  private async onDeviceUpdate(event: DeviceEvent) {
    console.log('📱 Device updated for user:', event.user_id);
    
    // Refresh cached device keys
    await this.refreshDeviceKeys(event.user_id);
  }

  private reconnect() {
    const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
    this.reconnectAttempts++;
    
    console.log(`Reconnecting in ${delay}ms...`);
    setTimeout(() => this.connect(), delay);
  }

  close() {
    this.ws?.close();
  }
}
```

### React Hook Example

```typescript
import { useEffect, useState } from 'react';

function useWebSocket(userId: string) {
  const [isConnected, setIsConnected] = useState(false);
  const [lastEvent, setLastEvent] = useState<RealtimeEvent | null>(null);

  useEffect(() => {
    const ws = new WebSocket(`ws://localhost:8080/ws?user_id=${userId}`);

    ws.onopen = () => setIsConnected(true);
    ws.onclose = () => setIsConnected(false);
    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      setLastEvent(data);
    };

    return () => ws.close();
  }, [userId]);

  return { isConnected, lastEvent };
}

// Usage
function ChatComponent({ userId }) {
  const { isConnected, lastEvent } = useWebSocket(userId);

  useEffect(() => {
    if (lastEvent?.type === 'message') {
      // Handle new message
      fetchAndDisplayNewMessages(lastEvent.chat_id);
    }
  }, [lastEvent]);

  return (
    <div>
      <StatusIndicator connected={isConnected} />
      {/* Your chat UI */}
    </div>
  );
}
```

---

## Error Handling

### Connection Errors

```javascript
ws.onerror = (error) => {
  console.error('WebSocket error:', error);
  
  // Log for debugging
  logError({
    type: 'websocket_error',
    userId: currentUserId,
    timestamp: Date.now(),
    error: error.message
  });
};
```

### Message Parsing Errors

```javascript
ws.onmessage = (event) => {
  try {
    const data = JSON.parse(event.data);
    handleEvent(data);
  } catch (error) {
    console.error('Failed to parse WebSocket message:', error);
    // Continue listening for next message
  }
};
```

### Stale Connection Detection

```javascript
let pingInterval;

ws.onopen = () => {
  // Send ping every 30 seconds
  pingInterval = setInterval(() => {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'ping' }));
    }
  }, 30000);
};

ws.onclose = () => {
  clearInterval(pingInterval);
};
```

---

## Performance Considerations

### Scalability

**Current Implementation**:
- Single PostgreSQL LISTEN connection
- In-memory broadcast channel
- All events broadcast to all handlers (filtered per-user)

**Scaling Strategies**:

1. **Redis Pub/Sub**: Replace PG LISTEN with Redis for multi-server deployments
2. **Event Filtering**: Filter events at database level (user-specific channels)
3. **Connection Pooling**: Limit concurrent WebSocket connections per server
4. **Load Balancing**: Use sticky sessions for WebSocket connections

### Memory Management

```rust
// Limit broadcast channel buffer size
let (tx, _rx) = broadcast::channel::<RealtimeEvent>(100);

// Receivers will lag if buffer overflows
// Implement lag handling in WebSocket handlers
```

### Network Optimization

- **JSON compression**: Consider binary protocols (MessagePack, Protocol Buffers)
- **Event batching**: Batch multiple events if high frequency
- **Selective updates**: Only send minimal diff information

---

## Best Practices

### Client-Side

1. **Implement exponential backoff** for reconnection
2. **Handle out-of-order events** gracefully
3. **Validate event payloads** before processing
4. **Log connection state changes** for debugging
5. **Show connection status** in UI

### Server-Side

1. **Rate limit** WebSocket connections per IP/user
2. **Authenticate** WebSocket connections properly
3. **Monitor** connection counts and memory usage
4. **Implement timeouts** for idle connections
5. **Graceful shutdown**: Close connections cleanly

### Testing

```javascript
// Mock WebSocket for testing
class MockWebSocket {
  constructor() {
    this.listeners = {};
  }

  addEventListener(event, callback) {
    this.listeners[event] = callback;
  }

  simulateMessage(data) {
    this.listeners.message({ data: JSON.stringify(data) });
  }
}

// Test event handling
test('handles new message event', () => {
  const mockWs = new MockWebSocket();
  const client = new HushNetClient(mockWs);
  
  mockWs.simulateMessage({
    type: 'message',
    chat_id: 'test-chat-id',
    user_id: 'test-user-id'
  });
  
  expect(client.fetchPendingMessages).toHaveBeenCalled();
});
```

---

## Future Improvements

- [ ] Binary protocol support (MessagePack/Protobuf)
- [ ] Message compression
- [ ] Event priority queues
- [ ] Redis integration for horizontal scaling
- [ ] WebSocket authentication with signatures
- [ ] Heartbeat/keepalive mechanism
- [ ] Delivery receipts via WebSocket
- [ ] Typing indicators
- [ ] Presence information

---

[← Back to Main Documentation](../README.md)
