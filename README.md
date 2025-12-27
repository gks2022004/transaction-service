# Transaction Service

A production-ready Rust microservice for managing business accounts, transactions, and webhooks with secure API authentication.

## Features

- **API Authentication**: Secure access with SHA-256 hashed API keys
- **Account Management**: Create and manage business accounts with balances
- **Transactions**: Credit, debit, and transfer operations with atomic balance updates
- **Webhooks**: Reliable webhook delivery with retries and HMAC signatures
- **Idempotency**: Prevent duplicate transactions with idempotency keys
- **Rate Limiting**: Token bucket rate limiting per API key
- **Observability**: Structured logging and OpenTelemetry integration

## Quick Start

### Prerequisites

- Docker and Docker Compose
- (Optional) Rust 1.75+ for local development

### Run with Docker Compose

```bash
# Start the service and database
docker compose up --build

# The service will be available at http://localhost:8080
```

### Create Your First API Key

```bash
curl -X POST http://localhost:8080/admin/api-keys \
  -H "Content-Type: application/json" \
  -d '{"business_name": "My Business"}'
```

Save the returned `key` - it's only shown once!

### Create an Account

```bash
curl -X POST http://localhost:8080/api/v1/accounts \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"name": "Main Account", "currency": "USD"}'
```

### Create a Transaction

```bash
# Credit an account
curl -X POST http://localhost:8080/api/v1/transactions \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: $(uuidgen)" \
  -d '{
    "type": "credit",
    "destination_account_id": "ACCOUNT_ID",
    "amount_cents": 10000
  }'
```

## API Documentation

See [API.md](API.md) for complete API documentation.

## Architecture

<img width="622" height="599" alt="image" src="https://github.com/user-attachments/assets/6f9e7efd-a9ec-4cc2-9ece-a2078a2831bf" />


## Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | Required | PostgreSQL connection string |
| `HOST` | `127.0.0.1` | Server bind address |
| `PORT` | `8080` | Server port |
| `RUST_LOG` | `info` | Log level |
| `RATE_LIMIT_REQUESTS_PER_SECOND` | `100` | Rate limit refill rate |
| `RATE_LIMIT_BURST_SIZE` | `50` | Maximum burst size |
| `WEBHOOK_TIMEOUT_SECONDS` | `30` | Webhook HTTP timeout |
| `WEBHOOK_MAX_RETRIES` | `5` | Maximum webhook retry attempts |

## Development

### Local Setup

```bash
# Copy environment template
cp .env.example .env

# Start PostgreSQL
docker compose up -d db

# Run the service
cargo run
```

### Run Tests

```bash
cargo test
```

### Build for Production

```bash
cargo build --release
```

## Database Schema

The service uses PostgreSQL with the following tables:

- `api_keys` - API key authentication
- `accounts` - Business accounts with balances
- `transactions` - Transaction records
- `webhooks` - Webhook endpoint registrations
- `webhook_deliveries` - Webhook delivery tracking

See [migrations/001_initial_schema.sql](migrations/001_initial_schema.sql) for the complete schema.

## Security Considerations

1. **API Keys**: Stored as SHA-256 hashes, never in plaintext
2. **Webhook Signatures**: HMAC-SHA256 with constant-time comparison
3. **Rate Limiting**: Prevents abuse and DoS attacks
4. **Input Validation**: All inputs validated before processing
5. **SQL Injection**: Prevented via parameterized queries (SQLx)


