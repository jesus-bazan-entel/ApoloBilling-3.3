//! Password hash generator utility
//!
//! Usage: cargo run --example gen_hash -p apolo-auth
//!
//! This generates an Argon2id password hash for "admin123" that can be
//! inserted into the usuarios table for authentication.

use apolo_auth::PasswordService;

fn main() {
    let password = std::env::args().nth(1).unwrap_or_else(|| "admin123".to_string());

    let service = PasswordService::new();
    let hash = service.hash_password(&password).expect("Failed to hash password");

    println!("Password: {}", password);
    println!("Hash: {}", hash);
    println!();
    println!("SQL para insertar usuario admin:");
    println!("INSERT INTO usuarios (username, password, nombre, role, activo)");
    println!("VALUES ('admin', '{}', 'Administrador', 'admin', true);", hash);
}
