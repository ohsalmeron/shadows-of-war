//! Partner portal SDK hooks (Poki / CrazyGames). No-op when JS helpers are absent.

use crate::platform_identity::PlatformIdentity;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
fn window() -> Option<web_sys::Window> {
    web_sys::window()
}

#[cfg(target_arch = "wasm32")]
fn get_window_value(name: &str) -> Option<wasm_bindgen::JsValue> {
    let w = window()?;
    js_sys::Reflect::get(&w, &wasm_bindgen::JsValue::from_str(name)).ok()
}

#[cfg(target_arch = "wasm32")]
fn call_window_hook(name: &str) {
    let Some(val) = get_window_value(name) else {
        return;
    };
    if val.is_function() {
        let Ok(func) = val.dyn_into::<js_sys::Function>() else {
            return;
        };
        let _ = func.call0(&wasm_bindgen::JsValue::NULL);
    }
}

#[cfg(target_arch = "wasm32")]
fn call_window_hook_str(name: &str, arg: &str) {
    let Some(val) = get_window_value(name) else {
        return;
    };
    if val.is_function() {
        let Ok(func) = val.dyn_into::<js_sys::Function>() else {
            return;
        };
        let _ = func.call1(
            &wasm_bindgen::JsValue::NULL,
            &wasm_bindgen::JsValue::from_str(arg),
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn call_window_hook_str_ret_string(name: &str, arg: &str) -> Option<String> {
    let Some(val) = get_window_value(name) else {
        return None;
    };
    if val.is_function() {
        let Ok(func) = val.dyn_into::<js_sys::Function>() else {
            return None;
        };
        if let Ok(res) = func.call1(
            &wasm_bindgen::JsValue::NULL,
            &wasm_bindgen::JsValue::from_str(arg),
        ) {
            return res.as_string();
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn take_window_u64(name: &str) -> Option<u64> {
    let val = get_window_value(name)?;
    if val.is_null() || val.is_undefined() {
        return None;
    }
    if let Some(s) = val.as_string() {
        return s.parse().ok();
    }
    val.as_f64().map(|n| n as u64)
}

#[cfg(target_arch = "wasm32")]
fn take_window_bool(name: &str) -> bool {
    get_window_value(name)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn call_window_hook(_name: &str) {}

#[cfg(not(target_arch = "wasm32"))]
fn call_window_hook_str(_name: &str, _arg: &str) {}

#[cfg(not(target_arch = "wasm32"))]
fn call_window_hook_str_ret_string(_name: &str, _arg: &str) -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn take_window_u64(_name: &str) -> Option<u64> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn take_window_bool(_name: &str) -> bool {
    false
}

pub fn gameplay_start() {
    call_window_hook("SOW_portalGameplayStart");
}

pub fn gameplay_stop() {
    call_window_hook("SOW_portalGameplayStop");
}

pub fn load_stop() {
    call_window_hook("SOW_portalLoadStop");
}

/// Read platform identity set by the portal SDK during init.
pub fn load_identity(fallback_name: &str) -> PlatformIdentity {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(obj) = get_window_value("SOW_PLATFORM_IDENTITY") {
            if !obj.is_null() && !obj.is_undefined() {
                let provider = js_string_field(&obj, "provider").unwrap_or_else(|| "self".into());
                let display_name = js_string_field(&obj, "displayName")
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| fallback_name.to_string());
                let external_id = js_string_field(&obj, "externalId");
                let avatar_url = js_string_field(&obj, "avatarUrl");
                let name_locked = js_bool_field(&obj, "nameLocked").unwrap_or(false);
                let provider_static: &'static str = match provider.as_str() {
                    "crazygames" => "crazygames",
                    "poki" => "poki",
                    "gamecenter" => "gamecenter",
                    "playgames" => "playgames",
                    "steam" => "steam",
                    _ => "self",
                };
                return PlatformIdentity {
                    provider: provider_static,
                    display_name,
                    external_id,
                    avatar_url,
                    name_locked,
                };
            }
        }
    }
    PlatformIdentity::self_hosted(fallback_name.to_string())
}

#[cfg(target_arch = "wasm32")]
fn js_string_field(obj: &wasm_bindgen::JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(obj, &wasm_bindgen::JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_string())
}

#[cfg(target_arch = "wasm32")]
fn js_bool_field(obj: &wasm_bindgen::JsValue, key: &str) -> Option<bool> {
    js_sys::Reflect::get(obj, &wasm_bindgen::JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_bool())
}

pub fn take_pending_invite_lobby() -> Option<u64> {
    take_window_u64("SOW_PENDING_INVITE_LOBBY_ID")
}

pub fn take_host_private_pending() -> bool {
    let pending = take_window_bool("SOW_HOST_PRIVATE_PENDING");
    if pending {
        call_window_hook("SOW_portalClearHostPrivatePending");
    }
    pending
}

pub fn poll_pending_invite_lobby() -> Option<u64> {
    let id = take_window_u64("SOW_PENDING_INVITE_LOBBY_ID")?;
    call_window_hook("SOW_portalClearPendingInvite");
    Some(id)
}

pub fn update_room(lobby_id: u64, joinable: bool, build_version: &str) {
    let json = format!(
        r#"{{"roomId":"{lobby_id}","isJoinable":{joinable},"inviteParams":{{"lobbyId":"{lobby_id}","buildVersion":"{build_version}"}}}}"#
    );
    call_window_hook_str("SOW_portalUpdateRoom", &json);
}

pub fn invite_link(lobby_id: u64, build_version: &str) -> Option<String> {
    let json = format!(r#"{{"lobbyId":"{lobby_id}","buildVersion":"{build_version}"}}"#);
    call_window_hook_str_ret_string("SOW_portalInviteLink", &json)
}

pub fn left_room() {
    call_window_hook("SOW_portalLeftRoom");
}

pub fn apply_mute_audio_setting(muted: bool) {
    if muted {
        call_window_hook("SOW_portalMuteGameAudio");
    } else {
        call_window_hook("SOW_portalUnmuteGameAudio");
    }
}

pub fn poll_mute_audio_setting() -> Option<bool> {
    #[cfg(target_arch = "wasm32")]
    {
        get_window_value("SOW_PORTAL_MUTE_AUDIO").and_then(|v| v.as_bool())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

pub fn is_chat_disabled() -> bool {
    take_window_bool("SOW_DISABLE_CHAT")
}
