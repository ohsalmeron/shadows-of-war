use serde::{Deserialize, Serialize};

// 1. Single source of truth for all assets and their filenames.
#[macro_export]
macro_rules! all_assets {
    ($macro:path $(, $extra:tt)*) => {
        $macro! {
            $($extra,)*
            City => "city.webp",
            Factory => "factory.svg",
            Port => "port.svg",
            DefensePost => "defense_post.svg",
            SamLauncher => "sam_launcher.svg",
            MissileSilo => "missile_silo.svg",
            TradeShip => "trade_ship.svg",
            TransportShip => "transport_ship.svg",
            Battleship => "battleship.svg",
            Star => "star.svg",
            AtomBomb => "atombomb.png",
            HydrogenBomb => "hydrogenbomb.png",
            Mirv => "mirv.png",
            SamMissile => "sam_missile.png",
            NukeExplosion => "nuke_explosion.png"
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

            pub fn is_svg(self) -> bool {
                self.file_name().ends_with(".svg")
            }
        }
    };
}

// Expand the enum and methods!
all_assets!(define_enum_and_methods);

// 3. Registering assets macro.
#[macro_export]
macro_rules! register_game_assets {
    ($ctx:expr, $prefix:expr) => {
        $crate::all_assets!($crate::register_single_asset, $ctx, $prefix);
    };
}

#[macro_export]
macro_rules! register_single_asset {
    ($ctx:expr, $prefix:expr, $($variant:ident => $file:expr),* $(,)?) => {
        $(
            $ctx.include_bytes(concat!("bytes://", $file), include_bytes!(concat!($prefix, $file)).as_slice());
        )*
    };
}
