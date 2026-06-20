use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Civilization {
    #[default]
    Rome,
    Egypt,
    Vikings,
    China,
    Macedon,
    Mongols,
    Angevin,
    Gallic,
    Iceni,
    Maya,
    Sparta,
    France,
}

impl Civilization {
    pub const ALL: [Civilization; 12] = [
        Civilization::Rome,
        Civilization::Egypt,
        Civilization::Vikings,
        Civilization::China,
        Civilization::Macedon,
        Civilization::Mongols,
        Civilization::Angevin,
        Civilization::Gallic,
        Civilization::Iceni,
        Civilization::Maya,
        Civilization::Sparta,
        Civilization::France,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Civilization::Rome => "Roman Empire",
            Civilization::Egypt => "Egyptian Empire",
            Civilization::Vikings => "Norse Kingdom",
            Civilization::China => "Chinese Empire",
            Civilization::Macedon => "Macedonian Empire",
            Civilization::Mongols => "Mongol Horde",
            Civilization::Angevin => "Angevin Empire",
            Civilization::Gallic => "Gallic Tribes",
            Civilization::Iceni => "Iceni Kingdom",
            Civilization::Maya => "Maya Civilization",
            Civilization::Sparta => "Sparta",
            Civilization::France => "Kingdom of France",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Leader {
    #[default]
    Caesar,
    Cleopatra,
    Ragnar,
    SunTzu,
    Alexander,
    GenghisKhan,
    RichardTheLionheart,
    Vercingetorix,
    Boudica,
    LadySixSky,
    Leonidas,
    Napoleon,
}

impl Leader {
    pub const ALL: [Leader; 12] = [
        Leader::Caesar,
        Leader::Cleopatra,
        Leader::Ragnar,
        Leader::SunTzu,
        Leader::Alexander,
        Leader::GenghisKhan,
        Leader::RichardTheLionheart,
        Leader::Vercingetorix,
        Leader::Boudica,
        Leader::LadySixSky,
        Leader::Leonidas,
        Leader::Napoleon,
    ];

    pub fn menu_emoji(self) -> &'static str {
        match self {
            Leader::Caesar => "🏛️",
            Leader::Cleopatra => "👑",
            Leader::Ragnar => "🪓",
            Leader::SunTzu => "📜",
            Leader::Alexander => "🛡️",
            Leader::GenghisKhan => "🐺",
            Leader::RichardTheLionheart => "🦁",
            Leader::Vercingetorix => "⚔️",
            Leader::Boudica => "🔥",
            Leader::LadySixSky => "🌙",
            Leader::Leonidas => "🪖",
            Leader::Napoleon => "🎖️",
        }
    }

    pub fn filler_rgb(self) -> [f32; 3] {
        match self {
            Leader::Caesar => [0.75, 0.15, 0.18],
            Leader::Cleopatra => [0.85, 0.65, 0.15],
            Leader::Ragnar => [0.15, 0.35, 0.65],
            Leader::SunTzu => [0.15, 0.55, 0.42],
            Leader::Alexander => [0.22, 0.45, 0.78],
            Leader::GenghisKhan => [0.55, 0.42, 0.22],
            Leader::RichardTheLionheart => [0.72, 0.18, 0.15],
            Leader::Vercingetorix => [0.28, 0.52, 0.22],
            Leader::Boudica => [0.88, 0.42, 0.12],
            Leader::LadySixSky => [0.12, 0.58, 0.52],
            Leader::Leonidas => [0.62, 0.42, 0.22],
            Leader::Napoleon => [0.18, 0.28, 0.68],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Leader::Caesar => "Caesar",
            Leader::Cleopatra => "Cleopatra",
            Leader::Ragnar => "Ragnar",
            Leader::SunTzu => "Sun Tzu",
            Leader::Alexander => "Alexander",
            Leader::GenghisKhan => "Genghis Khan",
            Leader::RichardTheLionheart => "Richard the Lionheart",
            Leader::Vercingetorix => "Vercingetorix",
            Leader::Boudica => "Boudica",
            Leader::LadySixSky => "Lady Six Sky",
            Leader::Leonidas => "Leonidas",
            Leader::Napoleon => "Napoleon",
        }
    }

    pub fn troop_strength_multiplier(self) -> f64 {
        match self {
            Leader::Caesar => 1.10,
            _ => 1.0,
        }
    }

    pub fn perk_description(self) -> &'static str {
        match self {
            Leader::Caesar => {
                "Legions of Rome: Armies fight 10% stronger (lower losses, faster conquest)."
            }
            Leader::Cleopatra => "Gift of the Nile: Factory districts generate +50% Gold.",
            Leader::Ragnar => "Longship Raid: Ports generate +50% Gold.",
            Leader::SunTzu => "Art of War: Factory districts produce troops 20% faster.",
            Leader::Alexander => "Great Conquest: Territory-conquering troops expand 15% faster.",
            Leader::GenghisKhan => "Horde Momentum: Gain 10% of gold spent by defeated enemies.",
            Leader::RichardTheLionheart => {
                "Crusader Fortresses: City districts grant +50% max troop capacity."
            }
            Leader::Vercingetorix => "Hillfort Gaul: City districts generate +50% troop income.",
            Leader::Boudica => "Iceni Revolt: City districts generate +50% Gold.",
            Leader::LadySixSky => "Temple of the Sky: Factory districts generate +50% Gold.",
            Leader::Leonidas => "Spartan Phalanx: Armory districts grant +50% max troop capacity.",
            Leader::Napoleon => "Grande Armée: Territory-conquering troops expand 20% faster.",
        }
    }

    pub fn civilization(self) -> Civilization {
        match self {
            Leader::Caesar => Civilization::Rome,
            Leader::Cleopatra => Civilization::Egypt,
            Leader::Ragnar => Civilization::Vikings,
            Leader::SunTzu => Civilization::China,
            Leader::Alexander => Civilization::Macedon,
            Leader::GenghisKhan => Civilization::Mongols,
            Leader::RichardTheLionheart => Civilization::Angevin,
            Leader::Vercingetorix => Civilization::Gallic,
            Leader::Boudica => Civilization::Iceni,
            Leader::LadySixSky => Civilization::Maya,
            Leader::Leonidas => Civilization::Sparta,
            Leader::Napoleon => Civilization::France,
        }
    }
}

pub fn leader_for_civilization(civ: Civilization) -> Leader {
    match civ {
        Civilization::Rome => Leader::Caesar,
        Civilization::Egypt => Leader::Cleopatra,
        Civilization::Vikings => Leader::Ragnar,
        Civilization::China => Leader::SunTzu,
        Civilization::Macedon => Leader::Alexander,
        Civilization::Mongols => Leader::GenghisKhan,
        Civilization::Angevin => Leader::RichardTheLionheart,
        Civilization::Gallic => Leader::Vercingetorix,
        Civilization::Iceni => Leader::Boudica,
        Civilization::Maya => Leader::LadySixSky,
        Civilization::Sparta => Leader::Leonidas,
        Civilization::France => Leader::Napoleon,
    }
}
