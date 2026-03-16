//! Benchmarks for CDR handlers
//!
//! Run with: cargo bench --package apolo-api
//!
//! These benchmarks measure the performance of statistics calculations
//! and data transformations (not database queries).

use apolo_core::models::Cdr;
use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rust_decimal::Decimal;

/// Create a mock CDR for testing
fn create_mock_cdr(id: i64, answered: bool) -> Cdr {
    let now = Utc::now();
    Cdr {
        id,
        call_uuid: format!("uuid-{}", id),
        account_id: Some(123),
        caller_number: "51999888777".to_string(),
        called_number: "15551234567".to_string(),
        destination_prefix: Some("1555".to_string()),
        start_time: now,
        answer_time: if answered { Some(now) } else { None },
        end_time: now,
        duration: 60,
        billsec: if answered { 55 } else { 0 },
        rate_id: Some(1),
        rate_per_minute: Some(Decimal::new(5, 2)),
        cost: Some(Decimal::new(92, 2)),
        hangup_cause: if answered {
            "NORMAL_CLEARING".to_string()
        } else {
            "NO_ANSWER".to_string()
        },
        direction: "outbound".to_string(),
        freeswitch_server_id: None,
        reservation_id: None,
        created_at: now,
        processed_at: Some(now),
    }
}

/// Benchmark CDR to CdrResponse conversion
fn bench_cdr_conversion(c: &mut Criterion) {
    use apolo_api::dto::CdrResponse;

    let cdr = create_mock_cdr(1, true);

    c.bench_function("cdr_to_response_conversion", |b| {
        b.iter(|| {
            let _response = CdrResponse::from(black_box(cdr.clone()));
        });
    });
}

/// Benchmark CDR to CdrExportRow conversion
fn bench_export_conversion(c: &mut Criterion) {
    use apolo_api::dto::CdrExportRow;

    let cdr = create_mock_cdr(1, true);

    c.bench_function("cdr_to_export_row_conversion", |b| {
        b.iter(|| {
            let _row = CdrExportRow::from(black_box(cdr.clone()));
        });
    });
}

/// Benchmark bulk conversion for export
fn bench_bulk_export_conversion(c: &mut Criterion) {
    use apolo_api::dto::CdrExportRow;

    let mut group = c.benchmark_group("bulk_export_conversion");

    for size in [100, 1_000, 10_000].iter() {
        let cdrs: Vec<Cdr> = (0..*size).map(|i| create_mock_cdr(i, i % 2 == 0)).collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let _rows: Vec<CdrExportRow> = black_box(&cdrs)
                    .iter()
                    .cloned()
                    .map(CdrExportRow::from)
                    .collect();
            });
        });
    }

    group.finish();
}

/// Benchmark statistics calculation (in-memory)
fn bench_stats_calculation(c: &mut Criterion) {
    // Note: This would benchmark the statistics calculation function
    // For now, we'll benchmark a simplified version

    let mut group = c.benchmark_group("stats_calculation");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let cdrs: Vec<Cdr> = (0..*size)
            .map(|i| create_mock_cdr(i, i % 10 != 0)) // 90% ASR
            .collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                // Simplified stats calculation
                let total = black_box(&cdrs).len();
                let answered = black_box(&cdrs).iter().filter(|c| c.was_answered()).count();
                let total_cost: Decimal = black_box(&cdrs).iter().filter_map(|c| c.cost).sum();
                let _asr = if total > 0 {
                    Decimal::from(answered) / Decimal::from(total) * Decimal::from(100)
                } else {
                    Decimal::ZERO
                };
                let _avg_cost = if total > 0 {
                    total_cost / Decimal::from(total)
                } else {
                    Decimal::ZERO
                };
            });
        });
    }

    group.finish();
}

/// Benchmark JSON serialization
fn bench_json_serialization(c: &mut Criterion) {
    use apolo_api::dto::CdrResponse;

    let mut group = c.benchmark_group("json_serialization");

    for size in [10, 100, 1_000].iter() {
        let responses: Vec<CdrResponse> = (0..*size)
            .map(|i| CdrResponse::from(create_mock_cdr(i, true)))
            .collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let _json = serde_json::to_string(black_box(&responses)).unwrap();
            });
        });
    }

    group.finish();
}

/// Benchmark CSV line formatting
fn bench_csv_formatting(c: &mut Criterion) {
    use apolo_api::dto::CdrExportRow;

    let row = CdrExportRow::from(create_mock_cdr(1, true));

    c.bench_function("csv_line_formatting", |b| {
        b.iter(|| {
            let _csv_line = format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                black_box(&row.id),
                black_box(&row.call_uuid),
                black_box(&row.account_id),
                black_box(&row.caller),
                black_box(&row.callee),
                black_box(&row.destination),
                black_box(&row.start_time),
                black_box(&row.answer_time),
                black_box(&row.end_time),
                black_box(&row.duration),
                black_box(&row.billsec),
                black_box(&row.rate),
                black_box(&row.cost),
                black_box(&row.hangup_cause),
                black_box(&row.direction)
            );
        });
    });
}

/// Benchmark filtering operations
fn bench_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("filtering");

    for size in [1_000, 10_000, 100_000].iter() {
        let cdrs: Vec<Cdr> = (0..*size)
            .map(|i| {
                let mut cdr = create_mock_cdr(i, i % 2 == 0);
                // Vary directions
                if i % 3 == 0 {
                    cdr.direction = "inbound".to_string();
                }
                cdr
            })
            .collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("filter_answered", size), size, |b, _| {
            b.iter(|| {
                let _filtered: Vec<&Cdr> = black_box(&cdrs)
                    .iter()
                    .filter(|c| c.was_answered())
                    .collect();
            });
        });

        group.bench_with_input(BenchmarkId::new("filter_direction", size), size, |b, _| {
            b.iter(|| {
                let _filtered: Vec<&Cdr> = black_box(&cdrs)
                    .iter()
                    .filter(|c| c.direction == "outbound")
                    .collect();
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_cdr_conversion,
    bench_export_conversion,
    bench_bulk_export_conversion,
    bench_stats_calculation,
    bench_json_serialization,
    bench_csv_formatting,
    bench_filtering
);

criterion_main!(benches);
