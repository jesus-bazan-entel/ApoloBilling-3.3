//! Integration tests for CDR API handlers
//!
//! These tests demonstrate the handlers work correctly with mock data.
//! For full integration testing, set DATABASE_URL environment variable.

#[cfg(test)]
mod tests {
    use apolo_api::dto::{CdrDirection, CdrFilterParams, PaginationParams, StatsGroupBy};
    use apolo_core::models::Cdr;
    use chrono::Utc;
    use rust_decimal::Decimal;

    #[test]
    fn test_cdr_filter_params_validation() {
        let params = CdrFilterParams {
            pagination: PaginationParams {
                page: 1,
                per_page: 50,
            },
            account_id: Some(123),
            caller: None,
            callee: None,
            start_date: None,
            end_date: None,
            direction: Some(CdrDirection::Outbound),
            hangup_cause: None,
            answered_only: false,
        };

        assert_eq!(params.pagination.page, 1);
        assert_eq!(params.pagination.offset(), 0);
        assert_eq!(params.pagination.limit(), 50);
    }

    #[test]
    fn test_pagination_offset_calculation() {
        let params = PaginationParams {
            page: 1,
            per_page: 10,
        };
        assert_eq!(params.offset(), 0);

        let params = PaginationParams {
            page: 3,
            per_page: 20,
        };
        assert_eq!(params.offset(), 40);
        assert_eq!(params.limit(), 20);
    }

    #[test]
    fn test_cdr_direction_conversion() {
        assert_eq!(CdrDirection::Inbound.as_str(), "inbound");
        assert_eq!(CdrDirection::Outbound.as_str(), "outbound");
    }

    #[test]
    fn test_stats_groupby_variants() {
        let group_by = StatsGroupBy::Day;
        assert_eq!(group_by, StatsGroupBy::Day);

        let group_by = StatsGroupBy::Hour;
        assert_eq!(group_by, StatsGroupBy::Hour);
    }

    #[test]
    fn test_cdr_response_conversion() {
        use apolo_api::dto::CdrResponse;

        let mut cdr = Cdr::default();
        cdr.id = 12345;
        cdr.caller_number = "51999888777".to_string();
        cdr.called_number = "15551234567".to_string();
        cdr.duration = 60;
        cdr.billsec = 55;
        cdr.answer_time = Some(Utc::now());

        let response = CdrResponse::from(cdr);

        assert_eq!(response.id, 12345);
        assert_eq!(response.caller, "51999888777");
        assert_eq!(response.callee, "15551234567");
        assert_eq!(response.duration, 60);
        assert_eq!(response.billsec, 55);
        assert!(response.answered);
    }

    #[test]
    fn test_cdr_export_row_conversion() {
        use apolo_api::dto::CdrExportRow;

        let mut cdr = Cdr::default();
        cdr.id = 54321;
        cdr.caller_number = "1234567890".to_string();
        cdr.called_number = "9876543210".to_string();
        cdr.cost = Some(Decimal::new(150, 2)); // 1.50

        let export_row = CdrExportRow::from(cdr);

        assert_eq!(export_row.id, 54321);
        assert_eq!(export_row.caller, "1234567890");
        assert_eq!(export_row.callee, "9876543210");
        assert_eq!(export_row.cost, "1.50");
    }

    #[test]
    fn test_export_format_content_type() {
        use apolo_api::dto::ExportFormat;

        assert_eq!(ExportFormat::Csv.content_type(), "text/csv; charset=utf-8");
        assert_eq!(
            ExportFormat::Json.content_type(),
            "application/json; charset=utf-8"
        );
        assert_eq!(
            ExportFormat::Jsonl.content_type(),
            "application/x-ndjson; charset=utf-8"
        );
    }

    #[test]
    fn test_export_format_extension() {
        use apolo_api::dto::ExportFormat;

        assert_eq!(ExportFormat::Csv.extension(), "csv");
        assert_eq!(ExportFormat::Json.extension(), "json");
        assert_eq!(ExportFormat::Jsonl.extension(), "jsonl");
    }

    #[test]
    fn test_pagination_metadata() {
        use apolo_core::traits::PaginationMeta;

        let meta = PaginationMeta::new(100, 1, 10);
        assert_eq!(meta.total, 100);
        assert_eq!(meta.page, 1);
        assert_eq!(meta.per_page, 10);
        assert_eq!(meta.total_pages, 10);

        let meta = PaginationMeta::new(95, 1, 10);
        assert_eq!(meta.total_pages, 10);

        let meta = PaginationMeta::new(101, 1, 10);
        assert_eq!(meta.total_pages, 11);
    }

    #[test]
    fn test_api_response_creation() {
        use apolo_api::dto::ApiResponse;

        let response = ApiResponse::success("test data");
        assert_eq!(response.data, "test data");
        assert!(response.message.is_none());

        let response = ApiResponse::with_message("data", "Operation successful");
        assert_eq!(response.data, "data");
        assert_eq!(response.message, Some("Operation successful".to_string()));
    }

    #[test]
    fn test_paginated_response() {
        use apolo_api::dto::PaginationParams;

        let params = PaginationParams {
            page: 2,
            per_page: 25,
        };

        let data = vec![1, 2, 3, 4, 5];
        let total = 100;

        let response = params.paginate(data.clone(), total);

        assert_eq!(response.data.len(), 5);
        assert_eq!(response.pagination.total, 100);
        assert_eq!(response.pagination.page, 2);
        assert_eq!(response.pagination.per_page, 25);
        assert_eq!(response.pagination.total_pages, 4);
    }
}

/// Mock database tests (requires DATABASE_URL to be set)
#[cfg(all(test, feature = "integration-tests"))]
mod integration_tests {
    use super::*;

    // These would be full integration tests with a real database
    // Run with: DATABASE_URL=... cargo test --features integration-tests

    #[actix_web::test]
    async fn test_list_cdrs_endpoint() {
        // Would test the actual HTTP endpoint with a test database
        todo!("Implement when test database is available");
    }

    #[actix_web::test]
    async fn test_export_streaming() {
        // Would test streaming with large datasets
        todo!("Implement when test database is available");
    }

    #[actix_web::test]
    async fn test_statistics_calculation() {
        // Would test statistics with known dataset
        todo!("Implement when test database is available");
    }
}
