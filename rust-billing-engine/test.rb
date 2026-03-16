use rust_decimal::Decimal;
use tokio_postgres::types::ToSql;

fn test_decimal_to_sql() {
    let d = Decimal::from_str("123.4567").unwrap();
    let _ = d as &dyn ToSql; // Si compila → la feature está activa
}
