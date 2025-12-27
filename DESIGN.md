# Transaction Service - Design Specification

A production-ready microservice for managing business accounts, transactions, and webhooks with secure API authentication.

## Problem Statement

Businesses need a reliable, secure system to:
- Track account balances across multiple accounts
- Process financial transactions (credits, debits, transfers) atomically
- Receive real-time notifications of transaction events via webhooks
- Operate at scale with proper rate limiting and observability

## Assumptions

1. **Single Currency Per Account**: Each account operates in a single currency (default: USD). Cross-currency transfers are out of scope.
2. **Non-Negative Balances**: Accounts cannot have negative balances; debit/transfer operations fail if insufficient funds.
3. **Business Isolation**: Each API key represents a business. Accounts belong to a single business and cannot be accessed by other businesses.
4. **Atomic Transactions**: All balance updates must be atomic - partial updates are not acceptable.
5. **Webhook Best Effort**: Webhooks are delivered with retries but are not guaranteed to succeed (external endpoint availability).
6. **Integer Cents**: All monetary values are stored as integer cents to avoid floating-point precision issues.
7. **UUID Identifiers**: All entities use UUID v4 for globally unique, non-sequential identifiers.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                            Clients                                  │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                          API Layer (Axum)                           │
│  ┌──────────────┐  ┌────────────────┐  ┌─────────────────────────┐  │
│  │ Rate Limiter │  │   Auth Guard   │  │   Request Handlers      │  │
│  │ (per API key)│  │ (SHA-256 hash) │  │   + Input Validation    │  │
│  └──────────────┘  └────────────────┘  └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         Service Layer                               │
│  ┌────────────────┐  ┌───────────────────┐  ┌────────────────────┐  │
│  │ AccountService │  │TransactionService │  │  WebhookService    │  │
│  │ (CRUD, balance)│  │(atomic transfers) │  │(registration, HMAC)│  │
│  └────────────────┘  └───────────────────┘  └────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                    │
              ┌─────────────────────┼─────────────────────┐
              ▼                     ▼                     ▼
┌─────────────────────┐  ┌──────────────────┐  ┌─────────────────────┐
│    PostgreSQL       │  │Webhook Dispatcher│  │OpenTelemetry        │
│    (SQLx + Pool)    │  │(Background Task) │  │(Traces + Metrics)   │
└─────────────────────┘  └──────────────────┘  └─────────────────────┘
```

---

## API Design

### Design Principles

1. **RESTful**: Resources are nouns (`/accounts`, `/transactions`), operations via HTTP methods
2. **Versioned**: All endpoints under `/api/v1/` to allow future breaking changes
3. **Consistent Errors**: Uniform error response format with machine-readable codes
4. **Idempotent Writes**: POST operations support `Idempotency-Key` header for safe retries
5. **Bearer Authentication**: Standard `Authorization: Bearer <token>` header

### Endpoint Summary

| Method | Endpoint                          | Description                    |
|--------|-----------------------------------|--------------------------------|
| GET    | `/health`                         | Liveness check                 |
| GET    | `/ready`                          | Readiness check (with DB ping) |
| POST   | `/admin/api-keys`                 | Create API key                 |
| POST   | `/api/v1/accounts`                | Create account                 |
| GET    | `/api/v1/accounts`                | List accounts                  |
| GET    | `/api/v1/accounts/:id`            | Get account                    |
| GET    | `/api/v1/accounts/:id/balance`    | Get balance only               |
| GET    | `/api/v1/accounts/:id/transactions` | List account transactions    |
| POST   | `/api/v1/transactions`            | Create transaction             |
| GET    | `/api/v1/transactions/:id`        | Get transaction                |
| POST   | `/api/v1/webhooks`                | Register webhook               |
| GET    | `/api/v1/webhooks`                | List webhooks                  |
| GET    | `/api/v1/webhooks/:id`            | Get webhook                    |
| DELETE | `/api/v1/webhooks/:id`            | Delete webhook                 |

### Transaction Types

| Type     | Source Required | Destination Required | Effect                          |
|----------|-----------------|----------------------|---------------------------------|
| `credit` | No              | Yes                  | Adds funds to destination       |
| `debit`  | Yes             | No                   | Removes funds from source       |
| `transfer`| Yes            | Yes                  | Moves funds between accounts    |

---

## Database Schema

### Entity Relationship Diagram

```
┌──────────────┐     1:N     ┌──────────────┐     1:N     ┌──────────────┐
│   api_keys   │─────────────│   accounts   │─────────────│ transactions │
└──────────────┘             └──────────────┘             └──────────────┘
       │                                                         │
       │ 1:N                                                     │ 1:N
       ▼                                                         ▼
┌──────────────┐                                         ┌────────────────────┐
│   webhooks   │─────────────────────────────────────────│ webhook_deliveries │
└──────────────┘                     1:N                 └────────────────────┘
```

### Tables

#### `api_keys`
Stores business API credentials with hashed keys.

| Column        | Type           | Description                              |
|---------------|----------------|------------------------------------------|
| `id`          | UUID (PK)      | Unique identifier                        |
| `key_hash`    | VARCHAR(64)    | SHA-256 hash of the API key              |
| `key_prefix`  | VARCHAR(8)     | First 8 chars for identification         |
| `business_name`| VARCHAR(255)  | Human-readable business name             |
| `is_active`   | BOOLEAN        | Soft delete flag                         |
| `created_at`  | TIMESTAMPTZ    | Creation timestamp                       |
| `updated_at`  | TIMESTAMPTZ    | Last modified timestamp                  |

**Indexes**: `key_hash` (for authentication lookup), `is_active` (partial index)

#### `accounts`
Business accounts with balances.

| Column        | Type           | Description                              |
|---------------|----------------|------------------------------------------|
| `id`          | UUID (PK)      | Unique identifier                        |
| `api_key_id`  | UUID (FK)      | Owning business                          |
| `name`        | VARCHAR(255)   | Account name                             |
| `balance_cents`| BIGINT        | Balance in smallest currency unit        |
| `currency`    | VARCHAR(3)     | ISO 4217 currency code                   |
| `is_active`   | BOOLEAN        | Soft delete flag                         |
| `created_at`  | TIMESTAMPTZ    | Creation timestamp                       |
| `updated_at`  | TIMESTAMPTZ    | Last modified timestamp                  |

**Constraints**: `balance_cents >= 0` (non-negative balance)

#### `transactions`
Immutable transaction records.

| Column               | Type           | Description                        |
|----------------------|----------------|------------------------------------|
| `id`                 | UUID (PK)      | Unique identifier                  |
| `idempotency_key`    | UUID           | Optional client-provided key       |
| `api_key_id`         | UUID (FK)      | Owning business                    |
| `transaction_type`   | ENUM           | `credit`, `debit`, or `transfer`   |
| `source_account_id`  | UUID (FK)      | Source account (nullable)          |
| `destination_account_id`| UUID (FK)   | Destination account (nullable)     |
| `amount_cents`       | BIGINT         | Transaction amount (must be > 0)   |
| `status`             | ENUM           | `pending`, `completed`, or `failed`|
| `metadata`           | JSONB          | Client-provided metadata           |
| `error_message`      | TEXT           | Failure reason if applicable       |
| `created_at`         | TIMESTAMPTZ    | Creation timestamp                 |
| `updated_at`         | TIMESTAMPTZ    | Last modified timestamp            |

**Constraints**: 
- `amount_cents > 0`
- Valid account combinations per transaction type

#### `webhooks`
Registered webhook endpoints.

| Column     | Type         | Description                              |
|------------|--------------|------------------------------------------|
| `id`       | UUID (PK)    | Unique identifier                        |
| `api_key_id`| UUID (FK)   | Owning business                          |
| `url`      | VARCHAR(2048)| Endpoint URL                             |
| `secret`   | VARCHAR(64)  | HMAC signing key                         |
| `events`   | TEXT[]       | Subscribed event types                   |
| `is_active`| BOOLEAN      | Active status                            |
| `created_at`| TIMESTAMPTZ | Creation timestamp                       |
| `updated_at`| TIMESTAMPTZ | Last modified timestamp                  |

#### `webhook_deliveries`
Delivery tracking and retry management.

| Column            | Type         | Description                          |
|-------------------|--------------|--------------------------------------|
| `id`              | UUID (PK)    | Unique identifier                    |
| `webhook_id`      | UUID (FK)    | Target webhook                       |
| `transaction_id`  | UUID (FK)    | Related transaction                  |
| `status`          | ENUM         | `pending`, `success`, or `failed`    |
| `retry_count`     | INT          | Current retry attempt                |
| `last_error`      | TEXT         | Last error message                   |
| `last_response_code`| INT        | HTTP response code                   |
| `next_retry_at`   | TIMESTAMPTZ  | When to retry next                   |
| `created_at`      | TIMESTAMPTZ  | Creation timestamp                   |
| `delivered_at`    | TIMESTAMPTZ  | Successful delivery timestamp        |

---

## Webhook Design

### Event Flow

```
Transaction Created
        │
        ▼
┌───────────────────┐
│ Find active hooks │
│ with matching     │
│ event type        │
└───────────────────┘
        │
        ▼
┌───────────────────┐
│ Create delivery   │
│ records (pending) │
└───────────────────┘
        │
        ▼
┌───────────────────┐
│ Background worker │
│ polls pending     │
│ deliveries        │
└───────────────────┘
        │
        ▼
┌───────────────────┐        ┌───────────────────┐
│ HTTP POST with    │───────▶│ External Endpoint │
│ HMAC signature    │        │                   │
└───────────────────┘        └───────────────────┘
        │
        ▼
┌───────────────────┐
│ Update status     │
│ (success/retry)   │
└───────────────────┘
```

### Signature Verification

Webhooks are signed using HMAC-SHA256:

```
Signature = HMAC-SHA256(webhook_secret, request_body)
```

The signature is sent in the `X-Webhook-Signature` header.

### Retry Policy

Failed deliveries use exponential backoff:

| Attempt | Delay    |
|---------|----------|
| 1       | 2s       |
| 2       | 4s       |
| 3       | 8s       |
| 4       | 16s      |
| 5 (max) | 32s      |

After max retries, the delivery is marked as permanently failed.

### Event Types

| Event                   | Trigger                           |
|-------------------------|-----------------------------------|
| `transaction.completed` | Transaction successfully processed|
| `transaction.failed`    | Transaction failed                |

---

## Security Considerations

### API Key Security

1. **Hashed Storage**: API keys are never stored in plaintext. Only SHA-256 hashes are persisted.
2. **One-Time Display**: The raw API key is only returned once during creation.
3. **Prefix for Identification**: A prefix (e.g., `sk_live_`) helps identify keys without exposing them.

### Authentication Flow

```
Client Request
      │
      ▼
┌──────────────────────┐
│ Extract Bearer token │
│ from Authorization   │
└──────────────────────┘
      │
      ▼
┌──────────────────────┐
│ Compute SHA-256 hash │
└──────────────────────┘
      │
      ▼
┌──────────────────────┐
│ Look up hash in DB   │
│ (indexed query)      │
└──────────────────────┘
      │
      ▼
┌──────────────────────┐
│ Verify is_active     │
└──────────────────────┘
      │
      ▼
┌──────────────────────┐
│ Inject api_key into  │
│ request context      │
└──────────────────────┘
```

### Input Validation

- All inputs validated using the `validator` crate
- Amount must be positive integers
- Currency codes must be valid 3-letter ISO codes
- URLs validated for webhook endpoints

### SQL Injection Prevention

All database queries use SQLx parameterized queries - no string interpolation.

---

## Operational Considerations

### Rate Limiting

**Algorithm**: Token bucket per API key

| Parameter         | Default | Description                    |
|-------------------|---------|--------------------------------|
| Refill rate       | 100/sec | Tokens added per second        |
| Burst capacity    | 50      | Maximum concurrent tokens      |

**Implementation Details**:
- In-memory token buckets with periodic cleanup
- Returns `429 Too Many Requests` with `Retry-After: 1` header
- Rate limit state is per-instance (not distributed)

### Idempotency

**Purpose**: Prevent duplicate transactions from network retries

**Implementation**:
1. Client provides UUID in `Idempotency-Key` header
2. Before processing, check for existing transaction with same key
3. If found, return existing transaction (200 OK instead of 201 Created)
4. If not found, create new transaction with key stored

**Scope**: Idempotency keys are global (not per API key) for simplicity

### Health Checks

| Endpoint  | Purpose                   | Checks                       |
|-----------|---------------------------|------------------------------|
| `/health` | Liveness probe            | Process is running           |
| `/ready`  | Readiness probe           | Database connection healthy  |

### Logging & Observability

**Structured Logging**:
- Uses `tracing` with JSON output in production
- Log levels configurable via `RUST_LOG` environment variable
- Request IDs propagated through spans

**OpenTelemetry Integration**:
- Traces exported to OTLP endpoint when configured
- Metrics for request latency, error rates
- Requires `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable

### Configuration

| Variable                         | Required | Default      | Description                |
|----------------------------------|----------|--------------|----------------------------|
| `DATABASE_URL`                   | Yes      | -            | PostgreSQL connection URL  |
| `HOST`                           | No       | `127.0.0.1`  | Bind address               |
| `PORT`                           | No       | `8080`       | Listen port                |
| `RUST_LOG`                       | No       | `info`       | Log level                  |
| `RATE_LIMIT_REQUESTS_PER_SECOND` | No       | `100`        | Rate limit refill rate     |
| `RATE_LIMIT_BURST_SIZE`          | No       | `50`         | Max burst                  |
| `WEBHOOK_TIMEOUT_SECONDS`        | No       | `30`         | Webhook HTTP timeout       |
| `WEBHOOK_MAX_RETRIES`            | No       | `5`          | Max webhook retry attempts |
| `OTEL_EXPORTER_OTLP_ENDPOINT`    | No       | -            | OpenTelemetry collector    |

---

## Trade-offs

### In-Memory Rate Limiting

**Chosen**: In-memory token buckets
**Alternative**: Redis-backed distributed rate limiting

| Approach      | Pros                          | Cons                               |
|---------------|-------------------------------|------------------------------------|
| In-memory     | Simple, no external deps      | Not shared across instances        |
| Redis         | Distributed, consistent       | Additional infrastructure, latency |

**Rationale**: For single-instance deployments, in-memory is sufficient. Multi-instance deployments would need Redis or similar.

### Webhooks: Pull vs. Push

**Chosen**: Push-based webhooks with retries
**Alternative**: Poll-based transaction status API

| Approach | Pros                           | Cons                              |
|----------|--------------------------------|-----------------------------------|
| Push     | Real-time, lower client load   | Requires public endpoint          |
| Poll     | Simpler for clients            | Higher API load, delayed updates  |

**Rationale**: Push is industry standard for payment notifications.

### Balance as Column vs. Computed

**Chosen**: Materialized balance column
**Alternative**: Compute balance from transaction history

| Approach    | Pros                        | Cons                              |
|-------------|-----------------------------|-----------------------------------|
| Materialized| O(1) balance reads          | Must maintain consistency         |
| Computed    | Always accurate             | O(n) reads, slower for large history|

**Rationale**: Fast balance reads are critical for validation. Atomic updates ensure consistency.

### Soft Deletes

**Chosen**: `is_active` flag for soft deletes
**Alternative**: Hard deletes with cascading

| Approach    | Pros                        | Cons                              |
|-------------|-----------------------------|-----------------------------------|
| Soft delete | Audit trail, recoverable    | Data accumulation                 |
| Hard delete | Clean data                  | Loss of history                   |

**Rationale**: Financial systems require audit trails and recoverability.

### Single Database

**Chosen**: PostgreSQL for all data
**Alternative**: Separate databases for transactions vs. webhooks

**Rationale**: Simpler operations. PostgreSQL handles the expected load well. Could split if webhook delivery volume becomes problematic.

---

## Future Enhancements

1. **Distributed Rate Limiting**: Redis-backed for multi-instance deployments
2. **Multi-Currency Support**: Exchange rates and cross-currency transfers
3. **Transaction Reversals**: Void/refund capabilities
4. **Scheduled Transactions**: Future-dated transfers
5. **Webhook Event Replay**: Manual retry of failed deliveries
6. **Admin Dashboard**: UI for API key management and monitoring
7. **Batch Transactions**: Bulk processing for high-volume use cases

---

## Deployment

### Docker Compose

The service is designed for containerized deployment:

```bash
# Start all services (including PostgreSQL)
docker compose up --build

# Start with observability stack
docker compose --profile observability up --build
```

### Database Migrations

Migrations run automatically on startup via SQLx embedded migrations.

### Health Monitoring

- Use `/health` for Kubernetes liveness probes
- Use `/ready` for readiness probes (waits for DB connection)
- Configure appropriate intervals (10s check, 30s timeout recommended)

---

## References

- [API Documentation](./API.md) - Complete endpoint reference
- [README](./README.md) - Quick start guide
- [Migrations](./migrations/001_initial_schema.sql) - Database schema
