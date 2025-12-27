# Transaction Service API Documentation

## Base URL

```
http://localhost:8080
```

## Authentication

All API endpoints (except health checks and admin endpoints) require authentication using API keys.

Include the API key in the `Authorization` header:

```
Authorization: Bearer sk_live_your_api_key_here
```

## Error Responses

All errors follow a consistent format:

```json
{
  "error": {
    "code": "error_code",
    "message": "Human readable error message"
  }
}
```

### Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `unauthorized` | 401 | Missing or invalid Authorization header |
| `invalid_api_key` | 401 | API key not found or inactive |
| `forbidden` | 403 | Access denied to resource |
| `not_found` | 404 | Resource not found |
| `validation_error` | 400 | Invalid request data |
| `conflict` | 409 | Duplicate idempotency key |
| `insufficient_balance` | 422 | Account has insufficient funds |
| `rate_limit_exceeded` | 429 | Too many requests |
| `internal_error` | 500 | Internal server error |

---

## Health Endpoints

### GET /health

Basic health check.

**Response**: `200 OK`

```
OK
```

### GET /ready

Readiness check (verifies database connection).

**Response**: `200 OK`

```
OK
```

---

## Admin Endpoints

### POST /admin/api-keys

Create a new API key for a business.

> **Note**: In production, this endpoint should be protected with admin authentication.

**Request Body**:

```json
{
  "business_name": "My Business Inc."
}
```

**Response**: `201 Created`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "key": "sk_live_a1b2c3d4e5f6...",
  "key_prefix": "sk_live_",
  "business_name": "My Business Inc.",
  "created_at": "2024-01-15T10:30:00Z"
}
```

> **Important**: The `key` field is only returned once during creation. Store it securely!

---

## Account Endpoints

### POST /api/v1/accounts

Create a new account.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`

**Request Body**:

```json
{
  "name": "Operating Account",
  "currency": "USD"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | - | Account name (1-255 chars) |
| `currency` | string | No | `USD` | 3-letter ISO currency code |

**Response**: `201 Created`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "name": "Operating Account",
  "balance_cents": 0,
  "balance_formatted": "0.00 USD",
  "currency": "USD",
  "is_active": true,
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

### GET /api/v1/accounts

List all accounts.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`

**Query Parameters**:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | integer | 20 | Max results (1-100) |
| `offset` | integer | 0 | Pagination offset |

**Response**: `200 OK`

```json
{
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440001",
      "name": "Operating Account",
      "balance_cents": 100000,
      "balance_formatted": "1000.00 USD",
      "currency": "USD",
      "is_active": true,
      "created_at": "2024-01-15T10:30:00Z",
      "updated_at": "2024-01-15T10:35:00Z"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

### GET /api/v1/accounts/:id

Get account details.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`

**Response**: `200 OK`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "name": "Operating Account",
  "balance_cents": 100000,
  "balance_formatted": "1000.00 USD",
  "currency": "USD",
  "is_active": true,
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:35:00Z"
}
```

### GET /api/v1/accounts/:id/balance

Get account balance only.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`

**Response**: `200 OK`

```json
{
  "account_id": "550e8400-e29b-41d4-a716-446655440001",
  "balance_cents": 100000,
  "balance_formatted": "1000.00 USD",
  "currency": "USD",
  "as_of": "2024-01-15T10:35:00Z"
}
```

---

## Transaction Endpoints

### POST /api/v1/transactions

Create a new transaction.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`
- `Idempotency-Key: <UUID>` (optional but recommended)

**Request Body**:

```json
{
  "type": "credit",
  "destination_account_id": "550e8400-e29b-41d4-a716-446655440001",
  "amount_cents": 10000,
  "metadata": {
    "order_id": "ORD-12345",
    "description": "Payment received"
  }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | `credit`, `debit`, or `transfer` |
| `source_account_id` | UUID | Conditional | Required for `debit` and `transfer` |
| `destination_account_id` | UUID | Conditional | Required for `credit` and `transfer` |
| `amount_cents` | integer | Yes | Amount in cents (must be > 0) |
| `metadata` | object | No | Custom metadata (stored as JSON) |

#### Transaction Types

| Type | Required Fields | Description |
|------|-----------------|-------------|
| `credit` | `destination_account_id` | Add funds to an account |
| `debit` | `source_account_id` | Remove funds from an account |
| `transfer` | Both account IDs | Move funds between accounts |

**Response**: `201 Created`

```json
{
  "id": "660e8400-e29b-41d4-a716-446655440002",
  "idempotency_key": "770e8400-e29b-41d4-a716-446655440003",
  "type": "credit",
  "source_account_id": null,
  "destination_account_id": "550e8400-e29b-41d4-a716-446655440001",
  "amount_cents": 10000,
  "amount_formatted": "100.00",
  "status": "completed",
  "metadata": {
    "order_id": "ORD-12345",
    "description": "Payment received"
  },
  "error_message": null,
  "created_at": "2024-01-15T10:35:00Z"
}
```

> **Idempotency**: If you provide an `Idempotency-Key` header and a transaction with that key already exists, the existing transaction is returned with `200 OK` instead of creating a duplicate.

### GET /api/v1/transactions/:id

Get transaction details.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`

**Response**: `200 OK`

```json
{
  "id": "660e8400-e29b-41d4-a716-446655440002",
  "idempotency_key": null,
  "type": "transfer",
  "source_account_id": "550e8400-e29b-41d4-a716-446655440001",
  "destination_account_id": "550e8400-e29b-41d4-a716-446655440002",
  "amount_cents": 5000,
  "amount_formatted": "50.00",
  "status": "completed",
  "metadata": {},
  "error_message": null,
  "created_at": "2024-01-15T10:40:00Z"
}
```

### GET /api/v1/accounts/:id/transactions

List transactions for an account.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`

**Query Parameters**:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | integer | 20 | Max results (1-100) |
| `offset` | integer | 0 | Pagination offset |

**Response**: `200 OK`

```json
{
  "data": [
    {
      "id": "660e8400-e29b-41d4-a716-446655440002",
      "type": "credit",
      "amount_cents": 10000,
      "status": "completed",
      "created_at": "2024-01-15T10:35:00Z"
    }
  ],
  "total": -1,
  "limit": 20,
  "offset": 0
}
```

---

## Webhook Endpoints

### POST /api/v1/webhooks

Register a new webhook endpoint.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`

**Request Body**:

```json
{
  "url": "https://your-server.com/webhooks/transactions",
  "events": ["transaction.completed", "transaction.failed"]
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `url` | string | Yes | - | Webhook endpoint URL (must be HTTPS in production) |
| `events` | array | No | `["transaction.completed", "transaction.failed"]` | Events to subscribe to |

**Response**: `201 Created`

```json
{
  "id": "880e8400-e29b-41d4-a716-446655440004",
  "url": "https://your-server.com/webhooks/transactions",
  "secret": "a1b2c3d4e5f6...",
  "events": ["transaction.completed", "transaction.failed"],
  "is_active": true
}
```

> **Important**: The `secret` field is only returned once during creation. Use it to verify webhook signatures!

### GET /api/v1/webhooks

List all registered webhooks.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`

**Response**: `200 OK`

```json
[
  {
    "id": "880e8400-e29b-41d4-a716-446655440004",
    "url": "https://your-server.com/webhooks/transactions",
    "events": ["transaction.completed", "transaction.failed"],
    "is_active": true,
    "created_at": "2024-01-15T10:30:00Z"
  }
]
```

### GET /api/v1/webhooks/:id

Get webhook details.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`

**Response**: `200 OK`

### DELETE /api/v1/webhooks/:id

Delete (deactivate) a webhook.

**Headers**:
- `Authorization: Bearer YOUR_API_KEY`

**Response**: `204 No Content`

---

## Webhook Payloads

When a transaction is completed or fails, registered webhooks receive a POST request.

### Headers

| Header | Description |
|--------|-------------|
| `Content-Type` | `application/json` |
| `X-Webhook-Signature` | HMAC-SHA256 signature of the payload |
| `X-Webhook-Id` | Unique delivery ID |

### Payload

```json
{
  "id": "990e8400-e29b-41d4-a716-446655440005",
  "event": "transaction.completed",
  "created_at": "2024-01-15T10:35:00Z",
  "data": {
    "transaction_id": "660e8400-e29b-41d4-a716-446655440002",
    "type": "credit",
    "source_account_id": null,
    "destination_account_id": "550e8400-e29b-41d4-a716-446655440001",
    "amount_cents": 10000,
    "status": "completed",
    "created_at": "2024-01-15T10:35:00Z"
  }
}
```

### Verifying Signatures

To verify the webhook came from us:

1. Get the raw request body
2. Compute HMAC-SHA256 using your webhook secret
3. Compare with the `X-Webhook-Signature` header

**Example (Node.js)**:

```javascript
const crypto = require('crypto');

function verifySignature(payload, secret, signature) {
  const computed = crypto
    .createHmac('sha256', secret)
    .update(payload)
    .digest('hex');
  
  return crypto.timingSafeEqual(
    Buffer.from(computed),
    Buffer.from(signature)
  );
}
```

**Example (Python)**:

```python
import hmac
import hashlib

def verify_signature(payload: bytes, secret: str, signature: str) -> bool:
    computed = hmac.new(
        secret.encode(),
        payload,
        hashlib.sha256
    ).hexdigest()
    return hmac.compare_digest(computed, signature)
```

### Retry Policy

Failed webhook deliveries are automatically retried with exponential backoff:

| Retry | Delay |
|-------|-------|
| 1 | 2 seconds |
| 2 | 4 seconds |
| 3 | 8 seconds |
| 4 | 16 seconds |
| 5 | 32 seconds |

After 5 failed attempts, the delivery is marked as permanently failed.

---

## Rate Limiting

API requests are rate limited per API key using a token bucket algorithm.

**Default Limits**:
- 100 requests per second (refill rate)
- 50 request burst capacity

When rate limited, you'll receive:

```
HTTP/1.1 429 Too Many Requests
Retry-After: 1

{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Too many requests. Please slow down."
  }
}
```

Wait for the specified `Retry-After` duration before retrying.
