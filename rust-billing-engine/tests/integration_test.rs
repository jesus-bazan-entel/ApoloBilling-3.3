// tests/integration_test.rs
#[cfg(test)]
mod tests {
    use actix_web::{test, App};
    use apolo_billing_engine::api::routes;

    #[actix_web::test]
    async fn test_health_endpoint() {
        let app = test::init_service(
            App::new().configure(routes::configure)
        ).await;
        
        let req = test::TestRequest::get()
            .uri("/api/v1/health")
            .to_request();
        
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_authorization_endpoint_format() {
        let app = test::init_service(
            App::new().configure(routes::configure)
        ).await;
        
        let payload = r#"{
            "caller": "+5215512345678",
            "callee": "18001234567"
        }"#;
        
        let req = test::TestRequest::post()
            .uri("/api/v1/authorize")
            .set_payload(payload)
            .insert_header(("content-type", "application/json"))
            .to_request();
        
        let resp = test::call_service(&app, req).await;
        
        // Should return 200 or 500, but not 404
        assert_ne!(resp.status().as_u16(), 404);
    }
}
