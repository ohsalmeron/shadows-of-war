use egui::Context;
use serde::{Deserialize, Serialize};

/// Embed a file from the workspace-root [`assets/`] tree (resolved via `sow-core` manifest).
#[macro_export]
macro_rules! repo_asset_bytes {
    ($path:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../assets/",
            $path
        ))
    };
}

// 1. Single source of truth for all assets and their filenames.
#[macro_export]
macro_rules! all_assets {
    ($macro:path $(, $extra:tt)*) => {
        $macro! {
            $($extra,)*
            City => "city.svg",
            Factory => "factory.svg",
            Port => "port.svg",
            DefensePost => "defense_post.svg",
            MissileSilo => "missile_silo.svg",
            TradeShip => "trade_ship.png",
            TransportShip => "transport_ship.png",
            Battleship => "battleship.png",
            Star => "star.webp",
            Handshake => "handshake.svg",
            AtomBomb => "atombomb.png",
            SamMissile => "sam_missile.png"
        }
    };
}

// 2. Generate the enum and its methods with compile-time concatenated URIs.
macro_rules! define_enum_and_methods {
    ($($variant:ident => $file:expr),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum Asset {
            $($variant),*
        }

        impl Asset {
            pub fn file_name(self) -> &'static str {
                match self {
                    $(Asset::$variant => $file),*
                }
            }

            pub fn uri(self) -> &'static str {
                match self {
                    $(Asset::$variant => concat!("bytes://", $file)),*
                }
            }
        }
    };
}

all_assets!(define_enum_and_methods);

// 3. One static table of icon bytes (linked once) instead of per-caller include_bytes!.
macro_rules! define_game_icon_table {
    ($($variant:ident => $file:expr),* $(,)?) => {
        const GAME_ICON_FILES: &[(&str, &[u8])] = &[
            $(($file, $crate::repo_asset_bytes!(concat!("icons/", $file)))),*
        ];
    };
}

all_assets!(define_game_icon_table);

/// Register all game SVG/PNG icons into egui's bytes:// loader (single copy in .rodata).
pub fn register_game_assets(ctx: &Context) {
    for &(file, bytes) in GAME_ICON_FILES {
        let uri = format!("bytes://{file}");
        ctx.include_bytes(uri, bytes);
    }
}
