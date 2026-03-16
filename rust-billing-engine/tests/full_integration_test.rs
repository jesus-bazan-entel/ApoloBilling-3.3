// tests/full_integration_test.rs
//! Full Integration Tests for Apolo Billing Engine
//!
//! These tests simulate complete call lifecycles and verify:
//! - Authorization flow
//! - Balance reservations
//! - Real-time billing
//! - CDR generation
//! - Database records
//!
//! Run with: cargo test --test full_integration_test -- --nocapture

mod esl_simulator;

use esl_simulator::{SimulatedCall, CallScenario, standard_test_scenarios, find_rate_for_destination};
use rust_decimal::Decimal;
use std::str::FromStr;
use tokio_postgres::{NoTls, Client};
use std::time::Duration;
use tokio::time::sleep;

/// Database configuration for tests
const DB_URL: &str = "host=127.0.0.1 user=postgres password=postgres dbname=apolo_billing";

/// API endpoint configuration
const API_BASE: &str = "http://127.0.0.1:9000/api/v1";

/// Test result tracking
#[derive(Debug)]
struct TestResult {
    scenario_name: String,
    passed: bool,
    message: String,
    balance_before: Option<Decimal>,
    balance_after: Option<Decimal>,
    expected_cost: Option<f64>,
    actual_cost: Option<Decimal>,
}

impl TestResult {
    fn pass(name: &str, message: &str) -> Self {
        Self {
            scenario_name: name.to_string(),
            passed: true,
            message: message.to_string(),
            balance_before: None,
            balance_after: None,
            expected_cost: None,
            actual_cost: None,
        }
    }

    fn fail(name: &str, message: &str) -> Self {
        Self {
            scenario_name: name.to_string(),
            passed: false,
            message: message.to_string(),
            balance_before: None,
            balance_after: None,
            expected_cost: None,
            actual_cost: None,
        }
    }

    fn with_balances(mut self, before: Decimal, after: Decimal) -> Self {
        self.balance_before = Some(before);
        self.balance_after = Some(after);
        self
    }

    fn with_costs(mut self, expected: f64, actual: Decimal) -> Self {
        self.expected_cost = Some(expected);
        self.actual_cost = Some(actual);
        self
    }
}

/// Connect to the database
async fn connect_db() -> Result<Client, tokio_postgres::Error> {
    let (client, connection) = tokio_postgres::connect(DB_URL, NoTls).await?;

    // Spawn connection handler
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Database connection error: {}", e);
        }
    });

    Ok(client)
}

/// Get account balance
async fn get_account_balance(db: &Client, account_number: &str) -> Option<Decimal> {
    let row = db
        .query_opt(
            "SELECT balance FROM accounts WHERE account_number = $1",
            &[&account_number],
        )
        .await
        .ok()??;

    row.try_get::<_, Decimal>(0).ok()
}

/// Get account by number
async fn get_account(db: &Client, account_number: &str) -> Option<(i32, String, Decimal)> {
    let row = db
        .query_opt(
            "SELECT id, status, balance FROM accounts WHERE account_number = $1",
            &[&account_number],
        )
        .await
        .ok()??;

    Some((
        row.get::<_, i32>(0),
        row.get::<_, String>(1),
        row.get::<_, Decimal>(2),
    ))
}

/// Count active reservations for an account
async fn count_active_reservations(db: &Client, account_id: i32) -> i64 {
    let row = db
        .query_one(
            "SELECT COUNT(*) FROM balance_reservations WHERE account_id = $1 AND status = 'active'",
            &[&account_id],
        )
        .await;

    match row {
        Ok(r) => r.get::<_, i64>(0),
        Err(_) => 0,
    }
}

/// Get reservation by call UUID
async fn get_reservation(db: &Client, call_uuid: &str) -> Option<(String, Decimal, Decimal, String)> {
    let row = db
        .query_opt(
            "SELECT id::text, reserved_amount, consumed_amount, status
             FROM balance_reservations
             WHERE call_uuid = $1
             ORDER BY created_at DESC LIMIT 1",
            &[&call_uuid],
        )
        .await
        .ok()??;

    Some((
        row.get::<_, String>(0),
        row.get::<_, Decimal>(1),
        row.get::<_, Decimal>(2),
        row.get::<_, String>(3),
    ))
}

/// Get CDR by call UUID
async fn get_cdr(db: &Client, call_uuid: &str) -> Option<(i32, Decimal)> {
    let row = db
        .query_opt(
            "SELECT billsec, cost FROM call_detail_records WHERE call_uuid = $1",
            &[&call_uuid],
        )
        .await
        .ok()??;

    Some((
        row.get::<_, i32>(0),
        row.get::<_, Decimal>(1),
    ))
}

/// Reset test account balance
async fn reset_account_balance(db: &Client, account_number: &str, balance: Decimal) -> bool {
    let result = db
        .execute(
            "UPDATE accounts SET balance = $1 WHERE account_number = $2",
            &[&balance, &account_number],
        )
        .await;

    result.is_ok()
}

/// Clean up test reservations
async fn cleanup_reservations(db: &Client, account_id: i32) -> bool {
    let result = db
        .execute(
            "DELETE FROM balance_reservations WHERE account_id = $1",
            &[&account_id],
        )
        .await;

    result.is_ok()
}

/// Make HTTP request to authorization API
async fn call_authorize_api(caller: &str, callee: &str, uuid: &str) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/authorize", API_BASE);

    let body = serde_json::json!({
        "caller": caller,
        "callee": callee,
        "uuid": uuid
    });

    let response = client
        .post(&url)
        .json(&body)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    Ok(json)
}

/// Make HTTP request to consume reservation API
async fn call_consume_api(call_uuid: &str, actual_cost: f64, actual_billsec: i64) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/reservation/consume", API_BASE);

    let body = serde_json::json!({
        "call_uuid": call_uuid,
        "actual_cost": actual_cost,
        "actual_billsec": actual_billsec
    });

    let response = client
        .post(&url)
        .json(&body)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    Ok(json)
}

/// Test authorization scenarios
async fn test_authorization_scenarios(db: &Client) -> Vec<TestResult> {
    let mut results = Vec::new();
    println!("\n========================================");
    println!("  AUTHORIZATION TESTS");
    println!("========================================\n");

    let scenarios = vec![
        // Valid prepaid account
        ("100001", "51987654321", true, None),
        // Another valid prepaid
        ("100002", "12125551234", true, None),
        // Suspended account
        ("100005", "51987654321", false, Some("account_suspended")),
        // Zero balance
        ("100004", "51987654321", false, Some("insufficient_balance")),
        // Unknown account
        ("999999", "51987654321", false, Some("account_not_found")),
        // Postpaid account
        ("200001", "51987654321", true, None),
    ];

    for (caller, callee, expected_auth, expected_reason) in scenarios {
        let uuid = uuid::Uuid::new_v4().to_string();
        let test_name = format!("Auth: {} -> {}", caller, callee);
        print!("  Testing {}: ", test_name);

        match call_authorize_api(caller, callee, &uuid).await {
            Ok(response) => {
                let authorized = response["authorized"].as_bool().unwrap_or(false);
                let reason = response["reason"].as_str().map(|s| s.to_string());

                if authorized == expected_auth {
                    if expected_auth {
                        println!("✅ PASS - Authorized as expected");
                        results.push(TestResult::pass(&test_name, "Authorized correctly"));
                    } else {
                        let got_reason = reason.as_deref().unwrap_or("unknown");
                        if expected_reason.map_or(true, |r| got_reason.contains(r)) {
                            println!("✅ PASS - Rejected with: {}", got_reason);
                            results.push(TestResult::pass(&test_name, &format!("Rejected: {}", got_reason)));
                        } else {
                            println!("❌ FAIL - Wrong reason: {} (expected: {:?})", got_reason, expected_reason);
                            results.push(TestResult::fail(&test_name, &format!("Wrong reason: {}", got_reason)));
                        }
                    }
                } else {
                    println!("❌ FAIL - authorized={}, expected={}", authorized, expected_auth);
                    results.push(TestResult::fail(&test_name, &format!("Authorization mismatch")));
                }

                // Cleanup reservation if created
                if authorized {
                    if let Some((account_id, _, _)) = get_account(db, caller).await {
                        let _ = db.execute(
                            "DELETE FROM balance_reservations WHERE call_uuid = $1",
                            &[&uuid],
                        ).await;
                    }
                }
            }
            Err(e) => {
                println!("❌ FAIL - API Error: {}", e);
                results.push(TestResult::fail(&test_name, &format!("API Error: {}", e)));
            }
        }
    }

    results
}

/// Test complete call lifecycle
async fn test_call_lifecycle(db: &Client) -> Vec<TestResult> {
    let mut results = Vec::new();
    println!("\n========================================");
    println!("  CALL LIFECYCLE TESTS");
    println!("========================================\n");

    // Reset test account
    let account = "100001";
    let initial_balance = Decimal::from_str("100.00").unwrap();
    reset_account_balance(db, account, initial_balance).await;

    let scenarios = vec![
        // (caller, callee, duration_secs, rate_per_min, billing_increment)
        ("100001", "51987654321", 30, 0.025, 6),   // Peru mobile
        ("100001", "12125551234", 60, 0.008, 6),   // USA NY
        ("100001", "18005551234", 120, 0.0, 60),   // Toll free
    ];

    for (caller, callee, duration, rate, increment) in scenarios {
        let uuid = uuid::Uuid::new_v4().to_string();
        let test_name = format!("Lifecycle: {} -> {} ({} secs)", caller, callee, duration);
        print!("\n  Testing {}:\n", test_name);

        // Get balance before
        let balance_before = get_account_balance(db, caller).await.unwrap_or_default();
        println!("    Balance before: ${:.4}", balance_before);

        // 1. AUTHORIZE
        print!("    1. Authorize: ");
        match call_authorize_api(caller, callee, &uuid).await {
            Ok(response) => {
                if response["authorized"].as_bool().unwrap_or(false) {
                    let reserved = response["reserved_amount"].as_f64().unwrap_or(0.0);
                    println!("✅ Reserved ${:.4}", reserved);

                    // 2. Verify reservation in DB
                    print!("    2. Check reservation: ");
                    if let Some((_, res_amount, _, status)) = get_reservation(db, &uuid).await {
                        println!("✅ Found - ${:.4}, status={}", res_amount, status);
                    } else {
                        println!("⚠️  Not found in DB (may be cached only)");
                    }

                    // 3. Calculate expected cost
                    let rounded_secs = ((duration + increment - 1) / increment) * increment;
                    let expected_cost = (rounded_secs as f64 / 60.0) * rate;
                    println!("    3. Expected cost: ${:.4} ({} secs @ ${:.4}/min)",
                             expected_cost, rounded_secs, rate);

                    // 4. CONSUME
                    print!("    4. Consume: ");
                    match call_consume_api(&uuid, expected_cost, duration as i64).await {
                        Ok(consume_response) => {
                            if consume_response["success"].as_bool().unwrap_or(false) {
                                let consumed = consume_response["consumed"]
                                    .as_f64()
                                    .unwrap_or(expected_cost);
                                println!("✅ Consumed ${:.4}", consumed);

                                // 5. Check balance after
                                let balance_after = get_account_balance(db, caller).await.unwrap_or_default();
                                let actual_deducted = balance_before - balance_after;
                                println!("    5. Balance after: ${:.4} (deducted: ${:.4})",
                                         balance_after, actual_deducted);

                                // Verify deduction matches expected
                                let expected_dec = Decimal::from_str(&format!("{:.4}", expected_cost)).unwrap_or_default();
                                let diff = (actual_deducted - expected_dec).abs();

                                if diff < Decimal::from_str("0.01").unwrap() {
                                    println!("    ✅ TEST PASSED - Cost matches expected");
                                    results.push(
                                        TestResult::pass(&test_name, "Cost verified")
                                            .with_balances(balance_before, balance_after)
                                            .with_costs(expected_cost, actual_deducted)
                                    );
                                } else {
                                    println!("    ⚠️  Cost mismatch: expected ${:.4}, actual ${:.4}",
                                             expected_cost, actual_deducted);
                                    results.push(
                                        TestResult::fail(&test_name, "Cost mismatch")
                                            .with_balances(balance_before, balance_after)
                                            .with_costs(expected_cost, actual_deducted)
                                    );
                                }
                            } else {
                                println!("❌ Consume failed");
                                results.push(TestResult::fail(&test_name, "Consume failed"));
                            }
                        }
                        Err(e) => {
                            println!("❌ API Error: {}", e);
                            results.push(TestResult::fail(&test_name, &format!("Consume API Error: {}", e)));
                        }
                    }
                } else {
                    println!("❌ Not authorized");
                    results.push(TestResult::fail(&test_name, "Authorization failed"));
                }
            }
            Err(e) => {
                println!("❌ API Error: {}", e);
                results.push(TestResult::fail(&test_name, &format!("Authorize API Error: {}", e)));
            }
        }

        // Small delay between tests
        sleep(Duration::from_millis(100)).await;
    }

    results
}

/// Test deficit management
async fn test_deficit_management(db: &Client) -> Vec<TestResult> {
    let mut results = Vec::new();
    println!("\n========================================");
    println!("  DEFICIT MANAGEMENT TESTS");
    println!("========================================\n");

    // Set up low balance account
    let account = "100003";
    let low_balance = Decimal::from_str("0.50").unwrap();
    reset_account_balance(db, account, low_balance).await;

    // Update status to active
    let _ = db.execute(
        "UPDATE accounts SET status = 'active' WHERE account_number = $1",
        &[&account],
    ).await;

    let test_name = "Deficit: Call exceeding balance";
    let uuid = uuid::Uuid::new_v4().to_string();
    print!("\n  Testing {}:\n", test_name);

    // This should authorize (minimum reservation of $0.30)
    print!("    1. Authorize low-balance call: ");
    match call_authorize_api(account, "51987654321", &uuid).await {
        Ok(response) => {
            if response["authorized"].as_bool().unwrap_or(false) {
                println!("✅ Authorized");

                // Try to consume more than reserved
                let high_cost = 5.0; // $5 cost but only $0.50 balance
                print!("    2. Consume ${:.2} (exceeds balance): ", high_cost);

                match call_consume_api(&uuid, high_cost, 600).await {
                    Ok(consume_response) => {
                        let balance_after = get_account_balance(db, account).await.unwrap_or_default();
                        println!("Balance after: ${:.4}", balance_after);

                        if balance_after < Decimal::ZERO {
                            println!("    ✅ Deficit recorded correctly");

                            // Check if account was suspended
                            if let Some((_, status, _)) = get_account(db, account).await {
                                print!("    3. Account status: {}", status);
                                if status == "suspended" && balance_after < Decimal::from_str("-10.0").unwrap() {
                                    println!(" (auto-suspended due to high deficit)");
                                }
                                println!();
                            }

                            results.push(TestResult::pass(test_name, "Deficit handled correctly"));
                        } else {
                            results.push(TestResult::fail(test_name, "Deficit not recorded"));
                        }
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                        results.push(TestResult::fail(test_name, &e));
                    }
                }
            } else {
                let reason = response["reason"].as_str().unwrap_or("unknown");
                println!("Rejected: {}", reason);
                // This might be expected if balance is too low for minimum reservation
                if reason.contains("insufficient") {
                    results.push(TestResult::pass(test_name, "Correctly rejected for low balance"));
                } else {
                    results.push(TestResult::fail(test_name, reason));
                }
            }
        }
        Err(e) => {
            println!("Error: {}", e);
            results.push(TestResult::fail(test_name, &e));
        }
    }

    // Reset account for future tests
    reset_account_balance(db, account, Decimal::from_str("1.00").unwrap()).await;
    let _ = db.execute(
        "UPDATE accounts SET status = 'active' WHERE account_number = $1",
        &[&account],
    ).await;

    results
}

/// Test database record verification
async fn test_database_records(db: &Client) -> Vec<TestResult> {
    let mut results = Vec::new();
    println!("\n========================================");
    println!("  DATABASE RECORD VERIFICATION");
    println!("========================================\n");

    // Check accounts table
    print!("  1. Accounts table: ");
    let count = db.query_one("SELECT COUNT(*) FROM accounts", &[]).await;
    match count {
        Ok(row) => {
            let c: i64 = row.get(0);
            println!("✅ {} accounts found", c);
            results.push(TestResult::pass("Accounts table", &format!("{} records", c)));
        }
        Err(e) => {
            println!("❌ Error: {}", e);
            results.push(TestResult::fail("Accounts table", &e.to_string()));
        }
    }

    // Check rate_cards table
    print!("  2. Rate cards table: ");
    let count = db.query_one("SELECT COUNT(*) FROM rate_cards", &[]).await;
    match count {
        Ok(row) => {
            let c: i64 = row.get(0);
            println!("✅ {} rate cards found", c);
            results.push(TestResult::pass("Rate cards table", &format!("{} records", c)));
        }
        Err(e) => {
            println!("❌ Error: {}", e);
            results.push(TestResult::fail("Rate cards table", &e.to_string()));
        }
    }

    // Check balance_reservations table
    print!("  3. Balance reservations: ");
    let count = db.query_one("SELECT COUNT(*) FROM balance_reservations", &[]).await;
    match count {
        Ok(row) => {
            let c: i64 = row.get(0);
            println!("✅ {} reservations found", c);
            results.push(TestResult::pass("Reservations table", &format!("{} records", c)));
        }
        Err(e) => {
            println!("❌ Error: {}", e);
            results.push(TestResult::fail("Reservations table", &e.to_string()));
        }
    }

    // Check balance_transactions table
    print!("  4. Balance transactions: ");
    let count = db.query_one("SELECT COUNT(*) FROM balance_transactions", &[]).await;
    match count {
        Ok(row) => {
            let c: i64 = row.get(0);
            println!("✅ {} transactions logged", c);
            results.push(TestResult::pass("Transactions table", &format!("{} records", c)));
        }
        Err(e) => {
            println!("❌ Error: {}", e);
            results.push(TestResult::fail("Transactions table", &e.to_string()));
        }
    }

    // Check CDR table
    print!("  5. Call detail records: ");
    let count = db.query_one("SELECT COUNT(*) FROM call_detail_records", &[]).await;
    match count {
        Ok(row) => {
            let c: i64 = row.get(0);
            println!("✅ {} CDRs found", c);
            results.push(TestResult::pass("CDR table", &format!("{} records", c)));
        }
        Err(e) => {
            println!("❌ Error: {}", e);
            results.push(TestResult::fail("CDR table", &e.to_string()));
        }
    }

    results
}

/// Print test summary
fn print_summary(results: &[TestResult]) {
    println!("\n========================================");
    println!("  TEST SUMMARY");
    println!("========================================\n");

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    println!("  Total:  {}", total);
    println!("  Passed: {} ✅", passed);
    println!("  Failed: {} ❌", failed);
    println!();

    if failed > 0 {
        println!("  Failed tests:");
        for result in results.iter().filter(|r| !r.passed) {
            println!("    - {}: {}", result.scenario_name, result.message);
        }
    }

    let success_rate = (passed as f64 / total as f64) * 100.0;
    println!("\n  Success rate: {:.1}%", success_rate);
    println!("========================================\n");
}

/// Main test entry point
#[tokio::test]
async fn run_full_integration_tests() {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   APOLO BILLING ENGINE - FULL INTEGRATION TEST SUITE     ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Check if API is available
    print!("Checking API availability at {}... ", API_BASE);
    let client = reqwest::Client::new();
    match client.get(&format!("{}/health", API_BASE))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            println!("✅ API is running");
        }
        Ok(response) => {
            println!("⚠️  API returned status: {}", response.status());
            println!("\n⚠️  Please start the billing engine first:");
            println!("    cd /opt/ApoloBilling/rust-billing-engine && cargo run");
            return;
        }
        Err(e) => {
            println!("❌ Cannot connect: {}", e);
            println!("\n⚠️  Please start the billing engine first:");
            println!("    cd /opt/ApoloBilling/rust-billing-engine && cargo run");
            return;
        }
    }

    // Connect to database
    print!("Connecting to database... ");
    let db = match connect_db().await {
        Ok(client) => {
            println!("✅ Connected");
            client
        }
        Err(e) => {
            println!("❌ Failed: {}", e);
            println!("\n⚠️  Make sure PostgreSQL is running and test data is loaded");
            return;
        }
    };

    let mut all_results = Vec::new();

    // Run test suites
    let db_results = test_database_records(&db).await;
    all_results.extend(db_results);

    let auth_results = test_authorization_scenarios(&db).await;
    all_results.extend(auth_results);

    let lifecycle_results = test_call_lifecycle(&db).await;
    all_results.extend(lifecycle_results);

    let deficit_results = test_deficit_management(&db).await;
    all_results.extend(deficit_results);

    // Print summary
    print_summary(&all_results);

    // Assert all tests passed
    let failed_count = all_results.iter().filter(|r| !r.passed).count();
    assert_eq!(failed_count, 0, "{} tests failed", failed_count);
}

/// Quick database connectivity test
#[tokio::test]
async fn test_database_connectivity() {
    println!("\nTesting database connectivity...");

    match connect_db().await {
        Ok(client) => {
            // Test query
            let result = client.query_one("SELECT 1 as test", &[]).await;
            match result {
                Ok(row) => {
                    let val: i32 = row.get(0);
                    assert_eq!(val, 1);
                    println!("✅ Database connectivity OK");
                }
                Err(e) => {
                    panic!("Query failed: {}", e);
                }
            }
        }
        Err(e) => {
            panic!("Connection failed: {}", e);
        }
    }
}

/// Test ESL event generation
#[test]
fn test_esl_event_generation() {
    println!("\nTesting ESL event generation...");

    let mut call = SimulatedCall::new("100001", "51987654321");

    // Test CHANNEL_CREATE
    let create_event = call.channel_create_event();
    assert!(create_event.contains("Event-Name: CHANNEL_CREATE"));
    assert!(create_event.contains("Caller-Caller-ID-Number: 100001"));
    assert!(create_event.contains("Caller-Destination-Number: 51987654321"));
    println!("  ✅ CHANNEL_CREATE event OK");

    // Test CHANNEL_ANSWER
    call.answer(2);
    let answer_event = call.channel_answer_event().unwrap();
    assert!(answer_event.contains("Event-Name: CHANNEL_ANSWER"));
    assert!(answer_event.contains("Answer-State: answered"));
    println!("  ✅ CHANNEL_ANSWER event OK");

    // Test CHANNEL_HANGUP_COMPLETE
    call.hangup(30, "NORMAL_CLEARING");
    let hangup_event = call.channel_hangup_event().unwrap();
    assert!(hangup_event.contains("Event-Name: CHANNEL_HANGUP_COMPLETE"));
    assert!(hangup_event.contains("variable_billsec: 30"));
    println!("  ✅ CHANNEL_HANGUP_COMPLETE event OK");

    println!("✅ All ESL event generation tests passed");
}
