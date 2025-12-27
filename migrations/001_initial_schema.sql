-- Migration: 001_initial_schema
-- Description: Initial database schema for transaction service

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- API Keys table
CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    key_hash VARCHAR(64) NOT NULL UNIQUE,  -- SHA-256 hash of the API key
    key_prefix VARCHAR(8) NOT NULL,         -- First 8 chars for identification
    business_name VARCHAR(255) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_active ON api_keys(is_active) WHERE is_active = TRUE;

-- Accounts table
CREATE TABLE accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    api_key_id UUID NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    balance_cents BIGINT NOT NULL DEFAULT 0,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT balance_non_negative CHECK (balance_cents >= 0)
);

CREATE INDEX idx_accounts_api_key ON accounts(api_key_id);
CREATE INDEX idx_accounts_active ON accounts(is_active) WHERE is_active = TRUE;

-- Transaction types enum
CREATE TYPE transaction_type AS ENUM ('credit', 'debit', 'transfer');

-- Transaction status enum
CREATE TYPE transaction_status AS ENUM ('pending', 'completed', 'failed');

-- Transactions table
CREATE TABLE transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    idempotency_key UUID UNIQUE,  -- Optional, for idempotent requests
    api_key_id UUID NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    transaction_type transaction_type NOT NULL,
    source_account_id UUID REFERENCES accounts(id) ON DELETE SET NULL,
    destination_account_id UUID REFERENCES accounts(id) ON DELETE SET NULL,
    amount_cents BIGINT NOT NULL,
    status transaction_status NOT NULL DEFAULT 'pending',
    metadata JSONB DEFAULT '{}',
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT amount_positive CHECK (amount_cents > 0),
    CONSTRAINT valid_transaction_accounts CHECK (
        (transaction_type = 'credit' AND destination_account_id IS NOT NULL) OR
        (transaction_type = 'debit' AND source_account_id IS NOT NULL) OR
        (transaction_type = 'transfer' AND source_account_id IS NOT NULL AND destination_account_id IS NOT NULL)
    )
);

CREATE INDEX idx_transactions_api_key ON transactions(api_key_id);
CREATE INDEX idx_transactions_source ON transactions(source_account_id) WHERE source_account_id IS NOT NULL;
CREATE INDEX idx_transactions_destination ON transactions(destination_account_id) WHERE destination_account_id IS NOT NULL;
CREATE INDEX idx_transactions_created ON transactions(created_at DESC);
CREATE INDEX idx_transactions_idempotency ON transactions(idempotency_key) WHERE idempotency_key IS NOT NULL;

-- Webhooks table
CREATE TABLE webhooks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    api_key_id UUID NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    url VARCHAR(2048) NOT NULL,
    secret VARCHAR(64) NOT NULL,  -- HMAC signing key
    events TEXT[] NOT NULL DEFAULT ARRAY['transaction.completed', 'transaction.failed'],
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_webhooks_api_key ON webhooks(api_key_id);
CREATE INDEX idx_webhooks_active ON webhooks(is_active) WHERE is_active = TRUE;

-- Webhook deliveries table
CREATE TYPE webhook_delivery_status AS ENUM ('pending', 'success', 'failed');

CREATE TABLE webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    webhook_id UUID NOT NULL REFERENCES webhooks(id) ON DELETE CASCADE,
    transaction_id UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    status webhook_delivery_status NOT NULL DEFAULT 'pending',
    retry_count INT NOT NULL DEFAULT 0,
    last_error TEXT,
    last_response_code INT,
    next_retry_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at TIMESTAMPTZ
);

CREATE INDEX idx_webhook_deliveries_pending ON webhook_deliveries(next_retry_at) 
    WHERE status = 'pending';
CREATE INDEX idx_webhook_deliveries_webhook ON webhook_deliveries(webhook_id);
CREATE INDEX idx_webhook_deliveries_transaction ON webhook_deliveries(transaction_id);

-- Updated at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply updated_at triggers
CREATE TRIGGER update_api_keys_updated_at BEFORE UPDATE ON api_keys
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_accounts_updated_at BEFORE UPDATE ON accounts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_transactions_updated_at BEFORE UPDATE ON transactions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_webhooks_updated_at BEFORE UPDATE ON webhooks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Create a default API key for testing (key: test_sk_live_1234567890abcdef)
-- The hash is for demonstration - in production, use proper key generation
INSERT INTO api_keys (id, key_hash, key_prefix, business_name, is_active)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08', -- SHA-256 of 'test'
    'test_sk_',
    'Test Business',
    TRUE
);
