//! Player reports, user blocks, and moderation email dispatch.
//!
//! Reports are ownership-proofed by the caller (the reporter's
//! `auth_secret` is verified by the HTTP handler before this module runs)
//! and rate-limited per reporter per day. A report always activates a block:
//! the reporter's client hides the reported account and the block persists
//! until the reporter's account is deleted.
//!
//! The moderation mailbox is configuration-only (`SOW_MODERATION_EMAIL` plus
//! `SOW_SMTP_*`). It is never sent to clients, never defaulted, and never
//! committed — when unset, reports are stored and logged but no email goes
//! out, so a missing mail setup can never break the game or leak addresses.

use redis::AsyncCommands;
use serde::Serialize;

use crate::db::PlayerDb;

/// Closed taxonomy for report reasons. `other` requires free-text details.
pub const REPORT_REASONS: &[&str] = &[
    "cheating",
    "harassment",
    "hate_speech",
    "threats",
    "spam",
    "inappropriate_name",
    "exploiting",
    "other",
];

/// Moderation records outlive product analytics: operators need them for
/// appeals and repeat-offender review. Documented in `docs/legal/DATA-DELETION.md`.
const REPORT_TTL_SECS: i64 = 365 * 24 * 3600;
const REPORTS_INDEX: &str = "sow:reports:index";
const REPORTS_INDEX_CAP: isize = 5000;
const BLOCK_PREFIX: &str = "sow:blocks:";
const RATELIMIT_PREFIX: &str = "sow:reports:ratelimit:";
/// Max reports one account can file per UTC day. Abuse of the button is
/// itself a conduct violation; the cap keeps the mailbox usable.
pub const REPORTS_PER_DAY: i64 = 20;
const DETAILS_MAX_CHARS: usize = 500;
const MATCH_ID_MAX_LEN: usize = 64;

pub struct ReportInput {
    pub reporter_account_id: String,
    pub reported_account_id: String,
    pub reported_public_id: String,
    pub match_id: Option<String>,
    pub reason: String,
    pub details: Option<String>,
}

#[derive(Serialize, Debug)]
struct StoredReport {
    id: String,
    reporter_account_id: String,
    reported_account_id: String,
    reported_public_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    match_id: Option<String>,
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
    created_at: u64,
}

pub struct ReportOutcome {
    pub report_id: String,
    pub blocked: bool,
    pub email_sent: bool,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// File one report and activate the block. Returns the stored report id,
/// whether the block was recorded, and whether the moderation email went out.
pub async fn submit_report(
    db: &PlayerDb,
    input: ReportInput,
) -> Result<ReportOutcome, Box<dyn std::error::Error + Send + Sync>> {
    if !REPORT_REASONS.contains(&input.reason.as_str()) {
        return Err("unknown report reason".into());
    }
    if input.reason == "other"
        && input
            .details
            .as_deref()
            .map(str::trim)
            .map(str::is_empty)
            .unwrap_or(true)
    {
        return Err("details are required for reason 'other'".into());
    }
    let details = input.details.map(|text| {
        text.chars()
            .filter(|ch| !ch.is_control() || *ch == '\n')
            .take(DETAILS_MAX_CHARS)
            .collect::<String>()
            .trim()
            .to_string()
    });
    let match_id = input.match_id.filter(|id| {
        !id.trim().is_empty() && id.len() <= MATCH_ID_MAX_LEN && !id.chars().any(char::is_control)
    });
    if input.reporter_account_id == input.reported_account_id {
        return Err("cannot report your own account".into());
    }

    let mut con = db.client_conn().await?;
    let day = crate::events::utc_date_string();
    let limit_key = format!("{RATELIMIT_PREFIX}{day}:{}", input.reporter_account_id);
    let count: i64 = con.incr(&limit_key, 1).await.unwrap_or(REPORTS_PER_DAY + 1);
    let _: () = con.expire(&limit_key, 2 * 24 * 3600).await.unwrap_or(());
    if count > REPORTS_PER_DAY {
        return Err("report rate limit exceeded".into());
    }

    let report_id = format!("{:032x}", rand::random::<u128>());
    let report = StoredReport {
        id: report_id.clone(),
        reporter_account_id: input.reporter_account_id.clone(),
        reported_account_id: input.reported_account_id.clone(),
        reported_public_id: input.reported_public_id.clone(),
        match_id,
        reason: input.reason.clone(),
        details: details.clone(),
        created_at: now_secs(),
    };
    let json = serde_json::to_string(&report)?;
    let key = format!("sow:report:{report_id}");
    let _: () = con.set_ex(&key, json, REPORT_TTL_SECS as u64).await?;
    let _: () = con
        .zadd(REPORTS_INDEX, &report_id, now_secs() as i64)
        .await?;
    let _: () = con
        .zremrangebyrank(REPORTS_INDEX, 0, -(REPORTS_INDEX_CAP + 1))
        .await?;

    let block_key = format!("{BLOCK_PREFIX}{}", input.reporter_account_id);
    // Store both id kinds: clients identify other players by public profile
    // id, while reports resolve to canonical account ids. Stale members are
    // harmless (they match nothing) and vanish with the reporter's account.
    let _: () = con
        .sadd(
            &block_key,
            vec![
                input.reported_account_id.clone(),
                input.reported_public_id.clone(),
            ],
        )
        .await?;

    let email_sent = send_moderation_email(&report).await;

    Ok(ReportOutcome {
        report_id,
        blocked: true,
        email_sent,
    })
}

/// Blocked ids for this account (owner-only read; the handler verifies
/// ownership before calling). Contains both canonical account ids and public
/// profile ids so clients can match whatever identifier they render.
pub async fn blocked_ids(
    db: &PlayerDb,
    account_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut con = db.client_conn().await?;
    let key = format!("{BLOCK_PREFIX}{account_id}");
    Ok(con.smembers(&key).await?)
}

struct MailConfig {
    host: String,
    port: u16,
    user: String,
    pass: String,
    from: String,
    to: String,
}

fn mail_config() -> Option<MailConfig> {
    Some(MailConfig {
        host: std::env::var("SOW_SMTP_HOST").ok()?,
        port: std::env::var("SOW_SMTP_PORT")
            .ok()
            .and_then(|port| port.parse().ok())
            .unwrap_or(587),
        user: std::env::var("SOW_SMTP_USER").ok()?,
        pass: std::env::var("SOW_SMTP_PASS").ok()?,
        from: std::env::var("SOW_SMTP_FROM").ok().unwrap_or_else(|| {
            std::env::var("SOW_SMTP_USER").unwrap_or_else(|_| "noreply@localhost".into())
        }),
        // The moderation mailbox. Env-only on purpose: the address must never
        // reach game clients, and there is no safe default to commit.
        to: std::env::var("SOW_MODERATION_EMAIL").ok()?,
    })
}

/// Best-effort moderation email. Never fails the report: `false` means the
/// report is stored and logged but no email went out (setup missing or SMTP
/// error), and the operator picks it up from the report index / logs.
async fn send_moderation_email(report: &StoredReport) -> bool {
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let Some(config) = mail_config() else {
        log::warn!(
            "[moderation] report {} stored but no moderation mailbox configured",
            report.id
        );
        return false;
    };
    let body = format!(
        "Shadows of War player report\n\
         \n\
         report_id: {id}\n\
         reason: {reason}\n\
         reported_public_id: {public_id}\n\
         reported_account_id: {account_id}\n\
         reporter_account_id: {reporter}\n\
         match_id: {match_id}\n\
         created_at_unix: {created_at}\n\
         details: {details}\n\
         \n\
         The reporter has blocked this account client-side. Review the report\n\
         index (sow:reports:index) and take action per the Terms of Service.\n",
        id = report.id,
        reason = report.reason,
        public_id = report.reported_public_id,
        account_id = report.reported_account_id,
        reporter = report.reporter_account_id,
        match_id = report.match_id.as_deref().unwrap_or("-"),
        created_at = report.created_at,
        details = report.details.as_deref().unwrap_or("-"),
    );
    let message = match Message::builder()
        .from(
            config
                .from
                .parse()
                .unwrap_or_else(|_| "noreply@localhost".parse().unwrap()),
        )
        .to(match config.to.parse() {
            Ok(mailbox) => mailbox,
            Err(error) => {
                log::error!("[moderation] invalid moderation mailbox: {error}");
                return false;
            }
        })
        .subject(format!(
            "[SOW report] {} against {}",
            report.reason, report.reported_public_id
        ))
        .body(body)
    {
        Ok(message) => message,
        Err(error) => {
            log::error!(
                "[moderation] report {} email build failed: {error}",
                report.id
            );
            return false;
        }
    };
    let transport = if config.port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
    };
    let transport = match transport {
        Ok(transport) => transport
            .credentials(Credentials::new(config.user, config.pass))
            .port(config.port)
            .timeout(Some(std::time::Duration::from_secs(10)))
            .build(),
        Err(error) => {
            log::error!(
                "[moderation] report {} SMTP setup failed: {error}",
                report.id
            );
            return false;
        }
    };
    match transport.send(message).await {
        Ok(_) => {
            log::info!(
                "[moderation] report {} emailed to moderation mailbox",
                report.id
            );
            true
        }
        Err(error) => {
            log::error!("[moderation] report {} email failed: {error}", report.id);
            false
        }
    }
}
