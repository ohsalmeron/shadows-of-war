//! Product-analytics transport: a closed-taxonomy event queue flushed to
//! sow-database over HTTP. Fire-and-forget — analytics failures never affect
//! gameplay, and the wire protocol (bincode WebSocket) is untouched.

use serde_json::json;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const FLUSH_INTERVAL_MS: u64 = 10_000;
const FLUSH_THRESHOLD: usize = 20;
const MAX_BATCH: usize = 100;
const MAX_QUEUED: usize = 200;

struct Config {
    endpoint: String,
    portal: String,
    platform: String,
    build: String,
    locale: String,
    session_id: String,
}

struct QueueEntry {
    name: &'static str,
    ts_ms: u64,
    props: Option<serde_json::Value>,
}

static CONFIG: OnceLock<Config> = OnceLock::new();
static QUEUE: Mutex<Vec<QueueEntry>> = Mutex::new(Vec::new());
static LAST_FLUSH_MS: Mutex<u64> = Mutex::new(0);
static SEQ: AtomicU32 = AtomicU32::new(0);
static GAMEPLAY_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Capture the ingest endpoint and envelope fields once at boot.
pub fn configure(database_base: &str) {
    let cfg = Config {
        endpoint: format!("{}/event", database_base.trim_end_matches('/')),
        portal: js_global("SOW_PORTAL").unwrap_or_else(|| "site".to_string()),
        platform: platform_name(),
        build: crate::get_build_version(),
        locale: locale_name(),
        session_id: session_id(),
    };
    let _ = CONFIG.set(cfg);
    install_pagehide_flush();
}

#[cfg(target_arch = "wasm32")]
fn install_pagehide_flush() {
    use wasm_bindgen::JsCast;
    let Some(window) = web_sys::window() else {
        return;
    };
    let callback = wasm_bindgen::closure::Closure::wrap(Box::new(|| {
        flush_if_due(true);
    }) as Box<dyn FnMut()>);
    let _ = window.add_event_listener_with_callback("pagehide", callback.as_ref().unchecked_ref());
    callback.forget();
}

#[cfg(not(target_arch = "wasm32"))]
fn install_pagehide_flush() {}

pub fn track(name: &'static str) {
    track_with(name, serde_json::Value::Null);
}

pub fn track_with(name: &'static str, props: serde_json::Value) {
    let Ok(mut queue) = QUEUE.lock() else { return };
    if queue.len() >= MAX_QUEUED {
        queue.remove(0);
    }
    queue.push(QueueEntry {
        name,
        ts_ms: now_ms(),
        props: if props.is_null() || props.as_object().is_some_and(|o| o.is_empty()) {
            None
        } else {
            Some(props)
        },
    });
    let due = queue.len() >= FLUSH_THRESHOLD;
    drop(queue);
    if due {
        flush_if_due(true);
    }
}

pub fn gameplay_start() {
    if !GAMEPLAY_ACTIVE.swap(true, Ordering::Relaxed) {
        track("gameplay_start");
    }
}

pub fn gameplay_stop() {
    if GAMEPLAY_ACTIVE.swap(false, Ordering::Relaxed) {
        track("gameplay_stop");
    }
}

/// Cheap per-frame check; performs real work only when the interval elapsed.
pub fn flush_if_due(force: bool) {
    let Some(cfg) = CONFIG.get() else { return };
    let now = now_ms();
    let mut last = match LAST_FLUSH_MS.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if !force && now.saturating_sub(*last) < FLUSH_INTERVAL_MS {
        return;
    }
    let entries = {
        let mut queue = match QUEUE.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if queue.is_empty() {
            *last = now;
            return;
        }
        let take = queue.len().min(MAX_BATCH);
        queue.drain(..take).collect::<Vec<_>>()
    };
    *last = now;
    drop(last);

    let account_id = crate::anonymous_identity::load_account_id();
    let body = build_batch_body(cfg, &entries, account_id.as_deref());
    let mut request = ehttp::Request::post(&cfg.endpoint, body.into_bytes());
    request.headers.insert("Content-Type", "application/json");
    ehttp::fetch(request, move |result| match result {
        Ok(response) if response.ok => {}
        Ok(response) => {
            log::debug!("[analytics] flush rejected: HTTP {}", response.status);
            requeue(entries);
        }
        Err(error) => {
            log::debug!("[analytics] flush failed: {error}");
            requeue(entries);
        }
    });
}

fn requeue(entries: Vec<QueueEntry>) {
    let Ok(mut queue) = QUEUE.lock() else { return };
    queue.splice(0..0, entries);
    if queue.len() > MAX_QUEUED {
        queue.truncate(MAX_QUEUED);
    }
}

/// Pure batch serializer — unit-tested without network.
fn build_batch_body(cfg: &Config, entries: &[QueueEntry], account_id: Option<&str>) -> String {
    let events: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            let mut event = json!({
                "v": 1,
                "name": entry.name,
                "ts_ms": entry.ts_ms,
                "session_id": cfg.session_id,
                "portal": cfg.portal,
                "platform": cfg.platform,
                "build": cfg.build,
                "locale": cfg.locale,
            });
            if let Some(account_id) = account_id {
                event["account_id"] = json!(account_id);
            }
            if let Some(props) = &entry.props
                && !props.as_object().is_some_and(|object| object.is_empty())
            {
                event["props"] = props.clone();
            }
            event
        })
        .collect();
    json!({ "events": events }).to_string()
}

fn now_ms() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn session_id() -> String {
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:04x}", now_ms(), seq)
}

#[cfg(target_arch = "wasm32")]
fn js_global(name: &str) -> Option<String> {
    let window = web_sys::window()?;
    let key = wasm_bindgen::JsValue::from_str(name);
    js_sys::Reflect::get(&window, &key)
        .ok()
        .and_then(|v| v.as_string())
        .filter(|s| !s.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn js_global(_name: &str) -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
fn platform_name() -> String {
    "web".to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_name() -> String {
    std::env::consts::OS.to_string()
}

#[cfg(target_arch = "wasm32")]
fn locale_name() -> String {
    web_sys::window()
        .and_then(|window| window.navigator().language())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn locale_name() -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            endpoint: "http://db/event".to_string(),
            portal: "crazygames".to_string(),
            platform: "web".to_string(),
            build: "123".to_string(),
            locale: "en".to_string(),
            session_id: "abc".to_string(),
        }
    }

    #[test]
    fn batch_body_carries_envelope_and_props() {
        let cfg = config();
        let entries = vec![QueueEntry {
            name: "tutorial_step",
            ts_ms: 1_800_000_000_000,
            props: Some(json!({ "idx": 3 })),
        }];
        let body = build_batch_body(&cfg, &entries, Some("cafe"));
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let event = &parsed["events"][0];
        assert_eq!(event["name"], "tutorial_step");
        assert_eq!(event["portal"], "crazygames");
        assert_eq!(event["account_id"], "cafe");
        assert_eq!(event["props"]["idx"], 3);
    }

    #[test]
    fn empty_props_are_dropped_not_sent() {
        let cfg = config();
        let entries = vec![QueueEntry {
            name: "boot_start",
            ts_ms: 1,
            props: Some(json!({})),
        }];
        let body = build_batch_body(&cfg, &entries, None);
        assert!(!body.contains("props"));
        assert!(!body.contains("account_id"));
    }

    #[test]
    fn track_drops_oldest_beyond_queue_cap() {
        QUEUE.lock().unwrap().clear();
        for _ in 0..(MAX_QUEUED + 10) {
            track("load_stage");
        }
        let queue = QUEUE.lock().unwrap();
        assert_eq!(queue.len(), MAX_QUEUED);
        assert_eq!(queue[0].name, "load_stage");
        drop(queue);
        QUEUE.lock().unwrap().clear();
    }
}
