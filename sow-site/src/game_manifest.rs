use std::sync::OnceLock;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct GameManifest {
    pub js: String,
    pub wasm: String,
    pub build_ts: String,
    pub version: String,
}

static MANIFEST: OnceLock<GameManifest> = OnceLock::new();

pub fn game_manifest() -> &'static GameManifest {
    MANIFEST.get_or_init(load_manifest)
}

fn load_manifest() -> GameManifest {
    if let Ok(raw) = std::env::var("SOW_GAME_MANIFEST") {
        if let Ok(m) = serde_json::from_str(&raw) {
            tracing::info!("game manifest from SOW_GAME_MANIFEST env");
            return m;
        }
    }

    let path = std::env::var("SOW_GAME_MANIFEST_PATH").unwrap_or_else(|_| {
        std::path::Path::new("sow-site/game-manifest.json")
            .exists()
            .then(|| "sow-site/game-manifest.json".into())
            .or_else(|| {
                std::path::Path::new("game-manifest.json")
                    .exists()
                    .then(|| "game-manifest.json".into())
            })
            .unwrap_or_else(|| "dist/play/game-manifest.json".into())
    });

    if let Ok(json) = std::fs::read_to_string(&path) {
        match serde_json::from_str(&json) {
            Ok(m) => {
                tracing::info!("game manifest loaded from {path}");
                return m;
            }
            Err(e) => tracing::warn!("invalid game manifest at {path}: {e}"),
        }
    } else {
        tracing::warn!("game manifest not found at {path} — Play embed disabled until built");
    }

    GameManifest {
        js: String::new(),
        wasm: String::new(),
        build_ts: String::new(),
        version: String::new(),
    }
}

pub fn manifest_json() -> String {
    serde_json::to_string(game_manifest()).unwrap_or_else(|_| "{}".into())
}

pub fn play_ready() -> bool {
    let m = game_manifest();
    !m.js.is_empty() && !m.wasm.is_empty()
}
