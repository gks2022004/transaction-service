#!/bin/bash

# Transaction Service - Comprehensive Test Script
# This script tests all API endpoints rigorously

set -e  # Exit on first error

BASE_URL="${BASE_URL:-http://localhost:8080}"
API_KEY=""
ACCOUNT_1_ID=""
ACCOUNT_2_ID=""
TRANSACTION_ID=""
WEBHOOK_ID=""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
TESTS_PASSED=0
TESTS_FAILED=0

print_header() {
    echo ""
    echo -e "${BLUE}================================================================${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}================================================================${NC}"
}

print_test() {
    echo -e "\n${YELLOW}TEST: $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ PASSED: $1${NC}"
    ((TESTS_PASSED++)) || true
}

print_failure() {
    echo -e "${RED}✗ FAILED: $1${NC}"
    echo -e "${RED}  Response: $2${NC}"
    ((TESTS_FAILED++)) || true
}

# Helper function to make HTTP requests and extract status/body
make_request() {
    local method=$1
    local endpoint=$2
    shift 2
    local response
    response=$(curl -s -w "|||%{http_code}" "$@" -X "$method" "$BASE_URL$endpoint")
    HTTP_CODE="${response##*|||}"
    BODY="${response%|||*}"
}

check_status() {
    local expected_status=$1
    local test_name=$2
    
    if [ "$HTTP_CODE" -eq "$expected_status" ]; then
        print_success "$test_name (HTTP $HTTP_CODE)"
        return 0
    else
        print_failure "$test_name (Expected HTTP $expected_status, got $HTTP_CODE)" "$BODY"
        return 1
    fi
}

# ================================================
# HEALTH CHECKS
# ================================================
print_header "HEALTH CHECK TESTS"

print_test "Health endpoint"
make_request GET "/health"
check_status 200 "Health check returns 200"

print_test "Readiness endpoint"
make_request GET "/ready"
check_status 200 "Readiness check returns 200"

# ================================================
# API KEY CREATION
# ================================================
print_header "API KEY TESTS"

print_test "Create API Key"
make_request POST "/admin/api-keys" \
    -H "Content-Type: application/json" \
    -d '{"business_name": "Test Business"}'

if check_status 201 "Create API key returns 201"; then
    API_KEY=$(echo "$BODY" | grep -o '"key":"[^"]*"' | cut -d'"' -f4)
    echo -e "  ${GREEN}API Key: ${API_KEY:0:20}...${NC}"
fi

if [ -z "$API_KEY" ]; then
    echo -e "${RED}FATAL: Could not create API key. Exiting.${NC}"
    exit 1
fi

# ================================================
# AUTHENTICATION TESTS
# ================================================
print_header "AUTHENTICATION TESTS"

print_test "Request without API key"
make_request GET "/api/v1/accounts"
check_status 401 "Unauthenticated request returns 401"

print_test "Request with invalid API key"
make_request GET "/api/v1/accounts" \
    -H "Authorization: Bearer invalid_key_12345"
check_status 401 "Invalid API key returns 401"

print_test "Request with valid API key"
make_request GET "/api/v1/accounts" \
    -H "Authorization: Bearer $API_KEY"
check_status 200 "Valid API key returns 200"

# ================================================
# ACCOUNT TESTS
# ================================================
print_header "ACCOUNT TESTS"

print_test "Create Account 1"
make_request POST "/api/v1/accounts" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{"name": "Test Account 1", "currency": "USD"}'

if check_status 201 "Create account 1 returns 201"; then
    ACCOUNT_1_ID=$(echo "$BODY" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
    echo -e "  ${GREEN}Account 1 ID: $ACCOUNT_1_ID${NC}"
fi

print_test "Create Account 2"
make_request POST "/api/v1/accounts" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{"name": "Test Account 2", "currency": "USD"}'

if check_status 201 "Create account 2 returns 201"; then
    ACCOUNT_2_ID=$(echo "$BODY" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
    echo -e "  ${GREEN}Account 2 ID: $ACCOUNT_2_ID${NC}"
fi

print_test "Create Account with invalid currency"
make_request POST "/api/v1/accounts" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{"name": "Bad Account", "currency": "INVALID"}'
check_status 400 "Invalid currency returns 400"

print_test "List accounts"
make_request GET "/api/v1/accounts" \
    -H "Authorization: Bearer $API_KEY"
if check_status 200 "List accounts returns 200"; then
    ACCOUNT_COUNT=$(echo "$BODY" | grep -o '"total":[0-9]*' | cut -d':' -f2)
    echo -e "  ${GREEN}Total accounts: $ACCOUNT_COUNT${NC}"
fi

print_test "Get Account 1"
make_request GET "/api/v1/accounts/$ACCOUNT_1_ID" \
    -H "Authorization: Bearer $API_KEY"
check_status 200 "Get account returns 200"

print_test "Get non-existent account"
make_request GET "/api/v1/accounts/00000000-0000-0000-0000-000000000000" \
    -H "Authorization: Bearer $API_KEY"
check_status 404 "Non-existent account returns 404"

print_test "Get Account 1 balance"
make_request GET "/api/v1/accounts/$ACCOUNT_1_ID/balance" \
    -H "Authorization: Bearer $API_KEY"
if check_status 200 "Get balance returns 200"; then
    BALANCE=$(echo "$BODY" | grep -o '"balance_cents":[0-9]*' | cut -d':' -f2)
    echo -e "  ${GREEN}Initial balance: $BALANCE cents${NC}"
fi

# ================================================
# TRANSACTION TESTS
# ================================================
print_header "TRANSACTION TESTS"

print_test "Credit Account 1 with \$100.00"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"type\": \"credit\", \"destination_account_id\": \"$ACCOUNT_1_ID\", \"amount_cents\": 10000}"
if check_status 201 "Credit transaction returns 201"; then
    TRANSACTION_ID=$(echo "$BODY" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
    echo -e "  ${GREEN}Transaction ID: $TRANSACTION_ID${NC}"
fi

print_test "Verify Account 1 balance after credit"
make_request GET "/api/v1/accounts/$ACCOUNT_1_ID/balance" \
    -H "Authorization: Bearer $API_KEY"
if check_status 200 "Balance check returns 200"; then
    BALANCE=$(echo "$BODY" | grep -o '"balance_cents":[0-9]*' | cut -d':' -f2)
    if [ "$BALANCE" -eq 10000 ]; then
        print_success "Balance is correct: $BALANCE cents"
    else
        print_failure "Balance incorrect: expected 10000, got $BALANCE" "$BODY"
    fi
fi

print_test "Credit Account 2 with \$50.00"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"type\": \"credit\", \"destination_account_id\": \"$ACCOUNT_2_ID\", \"amount_cents\": 5000}"
check_status 201 "Credit to account 2 returns 201"

print_test "Transfer \$25.00 from Account 1 to Account 2"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"type\": \"transfer\", \"source_account_id\": \"$ACCOUNT_1_ID\", \"destination_account_id\": \"$ACCOUNT_2_ID\", \"amount_cents\": 2500}"
check_status 201 "Transfer returns 201"

print_test "Verify Account 1 balance after transfer (should be \$75.00)"
make_request GET "/api/v1/accounts/$ACCOUNT_1_ID/balance" \
    -H "Authorization: Bearer $API_KEY"
BALANCE=$(echo "$BODY" | grep -o '"balance_cents":[0-9]*' | cut -d':' -f2)
if [ "$BALANCE" -eq 7500 ]; then
    print_success "Account 1 balance correct: $BALANCE cents (\$75.00)"
else
    print_failure "Account 1 balance incorrect: expected 7500, got $BALANCE" "$BODY"
fi

print_test "Verify Account 2 balance after transfer (should be \$75.00)"
make_request GET "/api/v1/accounts/$ACCOUNT_2_ID/balance" \
    -H "Authorization: Bearer $API_KEY"
BALANCE=$(echo "$BODY" | grep -o '"balance_cents":[0-9]*' | cut -d':' -f2)
if [ "$BALANCE" -eq 7500 ]; then
    print_success "Account 2 balance correct: $BALANCE cents (\$75.00)"
else
    print_failure "Account 2 balance incorrect: expected 7500, got $BALANCE" "$BODY"
fi

print_test "Debit \$10.00 from Account 1"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"type\": \"debit\", \"source_account_id\": \"$ACCOUNT_1_ID\", \"amount_cents\": 1000}"
check_status 201 "Debit returns 201"

print_test "Verify Account 1 balance after debit (should be \$65.00)"
make_request GET "/api/v1/accounts/$ACCOUNT_1_ID/balance" \
    -H "Authorization: Bearer $API_KEY"
BALANCE=$(echo "$BODY" | grep -o '"balance_cents":[0-9]*' | cut -d':' -f2)
if [ "$BALANCE" -eq 6500 ]; then
    print_success "Account 1 balance after debit: $BALANCE cents (\$65.00)"
else
    print_failure "Account 1 balance incorrect: expected 6500, got $BALANCE" "$BODY"
fi

# ================================================
# INSUFFICIENT BALANCE TESTS
# ================================================
print_header "INSUFFICIENT BALANCE TESTS"

print_test "Attempt to debit more than balance"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"type\": \"debit\", \"source_account_id\": \"$ACCOUNT_1_ID\", \"amount_cents\": 999999}"
check_status 422 "Insufficient balance returns 422"

print_test "Verify balance unchanged after failed debit"
make_request GET "/api/v1/accounts/$ACCOUNT_1_ID/balance" \
    -H "Authorization: Bearer $API_KEY"
BALANCE=$(echo "$BODY" | grep -o '"balance_cents":[0-9]*' | cut -d':' -f2)
if [ "$BALANCE" -eq 6500 ]; then
    print_success "Balance unchanged after failed transaction: $BALANCE cents"
else
    print_failure "Balance should be 6500, got $BALANCE (atomicity issue!)" "$BODY"
fi

# ================================================
# IDEMPOTENCY TESTS
# ================================================
print_header "IDEMPOTENCY TESTS"

IDEMPOTENCY_KEY=$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || echo "test-idem-$(date +%s)")

print_test "Create transaction with idempotency key (first time)"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -H "Idempotency-Key: $IDEMPOTENCY_KEY" \
    -d "{\"type\": \"credit\", \"destination_account_id\": \"$ACCOUNT_1_ID\", \"amount_cents\": 500}"
FIRST_TX_ID=$(echo "$BODY" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
check_status 201 "First request creates transaction"

print_test "Retry with same idempotency key (should return existing)"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -H "Idempotency-Key: $IDEMPOTENCY_KEY" \
    -d "{\"type\": \"credit\", \"destination_account_id\": \"$ACCOUNT_1_ID\", \"amount_cents\": 500}"
SECOND_TX_ID=$(echo "$BODY" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
check_status 200 "Retry returns 200 (not 201)"

if [ "$FIRST_TX_ID" = "$SECOND_TX_ID" ]; then
    print_success "Same transaction ID returned (idempotency works)"
else
    print_failure "Different transaction IDs: $FIRST_TX_ID vs $SECOND_TX_ID" "$BODY"
fi

print_test "Verify balance only increased once (\$5.00, not \$10.00)"
make_request GET "/api/v1/accounts/$ACCOUNT_1_ID/balance" \
    -H "Authorization: Bearer $API_KEY"
BALANCE=$(echo "$BODY" | grep -o '"balance_cents":[0-9]*' | cut -d':' -f2)
# Should be 6500 + 500 = 7000
if [ "$BALANCE" -eq 7000 ]; then
    print_success "Balance correct (idempotency prevented double-credit): $BALANCE cents"
else
    print_failure "Balance unexpected: got $BALANCE, expected 7000" "$BODY"
fi

# ================================================
# TRANSACTION RETRIEVAL TESTS
# ================================================
print_header "TRANSACTION RETRIEVAL TESTS"

print_test "Get transaction by ID"
make_request GET "/api/v1/transactions/$TRANSACTION_ID" \
    -H "Authorization: Bearer $API_KEY"
check_status 200 "Get transaction returns 200"

print_test "Get non-existent transaction"
make_request GET "/api/v1/transactions/00000000-0000-0000-0000-000000000000" \
    -H "Authorization: Bearer $API_KEY"
check_status 404 "Non-existent transaction returns 404"

print_test "List account transactions"
make_request GET "/api/v1/accounts/$ACCOUNT_1_ID/transactions" \
    -H "Authorization: Bearer $API_KEY"
check_status 200 "List account transactions returns 200"

# ================================================
# WEBHOOK TESTS
# ================================================
print_header "WEBHOOK TESTS"

print_test "Register webhook"
make_request POST "/api/v1/webhooks" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{"url": "https://webhook.site/test-endpoint", "events": ["transaction.completed"]}'
if check_status 201 "Register webhook returns 201"; then
    WEBHOOK_ID=$(echo "$BODY" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
    WEBHOOK_SECRET=$(echo "$BODY" | grep -o '"secret":"[^"]*"' | cut -d'"' -f4)
    echo -e "  ${GREEN}Webhook ID: $WEBHOOK_ID${NC}"
    echo -e "  ${GREEN}Webhook Secret: ${WEBHOOK_SECRET:0:16}...${NC}"
fi

print_test "List webhooks"
make_request GET "/api/v1/webhooks" \
    -H "Authorization: Bearer $API_KEY"
check_status 200 "List webhooks returns 200"

print_test "Get webhook by ID"
make_request GET "/api/v1/webhooks/$WEBHOOK_ID" \
    -H "Authorization: Bearer $API_KEY"
check_status 200 "Get webhook returns 200"

print_test "Delete webhook"
make_request DELETE "/api/v1/webhooks/$WEBHOOK_ID" \
    -H "Authorization: Bearer $API_KEY"
check_status 204 "Delete webhook returns 204"

print_test "Verify webhook deleted"
make_request GET "/api/v1/webhooks/$WEBHOOK_ID" \
    -H "Authorization: Bearer $API_KEY"
check_status 404 "Deleted webhook returns 404"

# ================================================
# VALIDATION TESTS
# ================================================
print_header "VALIDATION TESTS"

print_test "Create transaction with zero amount"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"type\": \"credit\", \"destination_account_id\": \"$ACCOUNT_1_ID\", \"amount_cents\": 0}"
check_status 400 "Zero amount returns 400"

print_test "Create transaction with negative amount"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"type\": \"credit\", \"destination_account_id\": \"$ACCOUNT_1_ID\", \"amount_cents\": -100}"
check_status 400 "Negative amount returns 400"

print_test "Create credit without destination account"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{"type": "credit", "amount_cents": 100}'
check_status 400 "Credit without destination returns 400"

print_test "Create debit without source account"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{"type": "debit", "amount_cents": 100}'
check_status 400 "Debit without source returns 400"

print_test "Create transfer with same source and destination"
make_request POST "/api/v1/transactions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"type\": \"transfer\", \"source_account_id\": \"$ACCOUNT_1_ID\", \"destination_account_id\": \"$ACCOUNT_1_ID\", \"amount_cents\": 100}"
check_status 400 "Transfer to same account returns 400"

# ================================================
# PAGINATION TESTS
# ================================================
print_header "PAGINATION TESTS"

print_test "List accounts with limit"
make_request GET "/api/v1/accounts?limit=1" \
    -H "Authorization: Bearer $API_KEY"
check_status 200 "Pagination with limit returns 200"

print_test "List accounts with offset"
make_request GET "/api/v1/accounts?limit=1&offset=1" \
    -H "Authorization: Bearer $API_KEY"
check_status 200 "Pagination with offset returns 200"

# ================================================
# SUMMARY
# ================================================
print_header "TEST SUMMARY"

echo ""
echo -e "Total tests: $((TESTS_PASSED + TESTS_FAILED))"
echo -e "${GREEN}Passed: $TESTS_PASSED${NC}"
echo -e "${RED}Failed: $TESTS_FAILED${NC}"
echo ""

if [ "$TESTS_FAILED" -eq 0 ]; then
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}  ALL TESTS PASSED! ${NC}"
    echo -e "${GREEN}========================================${NC}"
    exit 0
else
    echo -e "${RED}========================================${NC}"
    echo -e "${RED}  SOME TESTS FAILED${NC}"
    echo -e "${RED}========================================${NC}"
    exit 1
fi
