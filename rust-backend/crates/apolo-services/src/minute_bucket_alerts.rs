//! Minute bucket consumption alert dispatcher
//!
//! Polls `minute_bucket_alerts` for pending 80%/100% consumption alerts and
//! sends them by email via SMTP. Runs as its own background loop, separate
//! from the real-time billing hot path in `rust-billing-engine` (which only
//! INSERTs pending alert rows — it never talks to an SMTP server itself).
//!
//! Configuration is via environment variables (never `system_settings`,
//! which is readable by any authenticated user over the API):
//! `SMTP_HOST`, `SMTP_PORT` (default 587), `SMTP_USERNAME`, `SMTP_PASSWORD`,
//! `SMTP_FROM_EMAIL`, `SMTP_FROM_NAME`. If `SMTP_HOST` is unset the
//! dispatcher logs once and exits — alerts stay queued (`status='pending'`)
//! and are visible in the backoffice even without email delivery configured.

use apolo_db::PgMinuteBucketRepository;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::PgPool;
use std::env;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const POLL_INTERVAL_SECS: u64 = 60;

fn build_mailer() -> Option<(AsyncSmtpTransport<Tokio1Executor>, String, String)> {
    let smtp_host = match env::var("SMTP_HOST") {
        Ok(h) if !h.trim().is_empty() => h,
        _ => {
            info!(
                "SMTP_HOST not set - minute bucket email alerts are disabled (alerts stay queued in minute_bucket_alerts)"
            );
            return None;
        }
    };

    let smtp_port: u16 = env::var("SMTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(587);
    let smtp_username = env::var("SMTP_USERNAME").unwrap_or_default();
    let smtp_password = env::var("SMTP_PASSWORD").unwrap_or_default();
    let from_email =
        env::var("SMTP_FROM_EMAIL").unwrap_or_else(|_| "billing@apolobilling.local".to_string());
    let from_name = env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "ApoloBilling".to_string());

    let builder = match AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_host) {
        Ok(b) => b,
        Err(e) => {
            error!("Invalid SMTP_HOST '{}': {} - alerts disabled", smtp_host, e);
            return None;
        }
    };

    let builder = builder.port(smtp_port);
    let builder = if !smtp_username.is_empty() {
        builder.credentials(Credentials::new(smtp_username, smtp_password))
    } else {
        builder
    };

    info!(
        "Minute bucket alert dispatcher ready (SMTP host: {}:{}, poll every {}s)",
        smtp_host, smtp_port, POLL_INTERVAL_SECS
    );

    Some((builder.build(), from_email, from_name))
}

fn alert_subject_body(threshold_pct: i16, account_number: &str) -> (String, String) {
    if threshold_pct >= 100 {
        (
            format!(
                "[ApoloBilling] Bolsa de minutos agotada - cuenta {}",
                account_number
            ),
            format!(
                "La cuenta {account_number} alcanzo el 100% de su bolsa de minutos del ciclo actual.\n\n\
                 Si el plan es Control, las llamadas salientes NUEVAS quedan bloqueadas hasta agregar \
                 una bolsa adicional o hasta el inicio del proximo ciclo. Las llamadas que ya estaban \
                 en curso no se cortan; sus minutos excedentes se facturan segun la tarifa de excedente \
                 configurada.\n\n\
                 Si el plan es Abierto, el trafico continua con normalidad y todo minuto adicional se \
                 factura como excedente.",
                account_number = account_number
            ),
        )
    } else {
        (
            format!(
                "[ApoloBilling] Bolsa de minutos al {}% - cuenta {}",
                threshold_pct, account_number
            ),
            format!(
                "La cuenta {} alcanzo el {}% de su bolsa de minutos del ciclo actual. \
                 Puede revisar el detalle en el backoffice de ApoloBilling.",
                account_number, threshold_pct
            ),
        )
    }
}

/// Runs forever, polling for pending alerts and sending them by email.
/// Returns (never loops) if SMTP is not configured, so callers should
/// `tokio::spawn` this rather than `.await` it inline.
pub async fn run_alert_dispatcher(pool: PgPool) {
    let Some((mailer, from_email, from_name)) = build_mailer() else {
        return;
    };

    let repo = PgMinuteBucketRepository::new(pool);
    let from_mailbox = format!("{} <{}>", from_name, from_email);

    loop {
        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;

        let pending = match repo.pending_alerts().await {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to read pending minute bucket alerts: {}", e);
                continue;
            }
        };

        if pending.is_empty() {
            continue;
        }

        debug!("Dispatching {} pending minute bucket alert(s)", pending.len());

        for (alert, account_number, notification_email) in pending {
            let Some(email) = notification_email else {
                debug!(
                    "Alert {} for account {} has no notification_email set, leaving pending",
                    alert.id, account_number
                );
                continue;
            };

            let to_mailbox = match email.parse() {
                Ok(addr) => addr,
                Err(e) => {
                    warn!(
                        "Invalid notification_email '{}' for account {}: {}",
                        email, account_number, e
                    );
                    let _ = repo
                        .mark_alert_failed(alert.id, &format!("Invalid email address: {}", e))
                        .await;
                    continue;
                }
            };

            let from = match from_mailbox.parse() {
                Ok(addr) => addr,
                Err(e) => {
                    error!("Invalid SMTP_FROM_EMAIL/SMTP_FROM_NAME '{}': {}", from_mailbox, e);
                    let _ = repo
                        .mark_alert_failed(alert.id, &format!("Invalid from address: {}", e))
                        .await;
                    continue;
                }
            };

            let (subject, body) = alert_subject_body(alert.threshold_pct, &account_number);

            let message = Message::builder()
                .from(from)
                .to(to_mailbox)
                .subject(subject)
                .header(ContentType::TEXT_PLAIN)
                .body(body);

            let message = match message {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to build alert email for {}: {}", alert.id, e);
                    let _ = repo
                        .mark_alert_failed(alert.id, &format!("Failed to build message: {}", e))
                        .await;
                    continue;
                }
            };

            match mailer.send(message).await {
                Ok(_) => {
                    info!(
                        "Sent {}% bucket alert for account {} to {}",
                        alert.threshold_pct, account_number, email
                    );
                    let _ = repo.mark_alert_sent(alert.id).await;
                }
                Err(e) => {
                    error!(
                        "Failed to send bucket alert {} for account {}: {}",
                        alert.id, account_number, e
                    );
                    let _ = repo.mark_alert_failed(alert.id, &e.to_string()).await;
                }
            }
        }
    }
}
