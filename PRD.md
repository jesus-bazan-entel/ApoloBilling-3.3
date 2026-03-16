# Product Requirements Document (PRD)
## Apolo Billing Engine (Rust)

### 1. Overview
The Apolo Billing Engine is a high-performance, real-time rating and billing system designed to integrate with FreeSWITCH via the Event Socket Layer (ESL). It manages authorization, balance reservations, and Call Detail Record (CDR) generation for prepaid and postpaid accounts.

### 2. Architecture
- **Language**: Rust (async/await with Tokio)
- **Database**: PostgreSQL (Persistent storage for Accounts, Rates, CDRs)
- **Cache**: Redis (Real-time session state, Balance reservations)
- **Integration**: FreeSWITCH ESL (Inbound connection)

### 3. Core Features

#### 3.1 Authorization (`AuthorizationService`)
- **Trigger**: `CHANNEL_CREATE` event.
- **Logic**:
    - Validates Caller ID (ANI) against `accounts` table.
    - Checks `AccountStatus` (Must be 'ACTIVE').
    - **Prepaid**: Verification of sufficient balance > 0.
    - **Postpaid**: Verification of credit limit.
- **Outcome**: Returns `true` (allow) or `false` (deny) with reason.

#### 3.2 Real-time Billing (`RealtimeBiller`)
- **Trigger**: `CHANNEL_ANSWER` event.
- **Logic**:
    - Starts a background task for each active call.
    - Reserves balance chunks in Redis/DB periodically.
    - Monitors call duration against available balance.

#### 3.3 CDR Generation (`CdrGenerator`)
- **Trigger**: `CHANNEL_HANGUP_COMPLETE` event.
- **Logic**:
    - Captures final metrics: `duration`, `billsec`, `hangup_cause`.
    - Retrieves final reservation cost.
    - **Fallback**: Generates a basic CDR (without billing details) if no valid reservation is found, ensuring calls are always logged.
    - **Consistency**: Inserts record into `cdrs` table (using `i64` IDs).
    - **Cleanup**: Consumes the balance reservation or clears residual Redis state if no reservation existed.

### 4. Data Models Updated
- **Account**: Updated to use `i64` for IDs and `Option<i32>` for nullable fields (`max_concurrent_calls`).
- **RateCard**: Updated to use `Option<DateTime<Utc>>` for nullable `effective_end`.

### 5. Integration Interfaces
- **ESL Events Handled**:
    - `CHANNEL_CREATE`: Auth
    - `CHANNEL_ANSWER`: Start Billing
    - `CHANNEL_HANGUP_COMPLETE`: End Billing
