//! The campaign **dialog database** — one home for every scripted line (tutorial + future
//! episodes), so content grows here without touching UI code. Each line is rendered through the
//! bottom-panel takeover modal (a `BottomDialog` painted by `paint_dialog_contents`); add freely.
//!
//! Lines go through the emoji atlas pipeline, so emoji render fine; keep lines short — the panel
//! is compact (see docs/campaign-tutorial.md §0).
//!
//! The speaker's *visual* (avatar / tribe emoji / empire disc) is built from the live snapshot at
//! render time, not stored here — this file is just the words.

/// One line of scripted speech (owned, so a generic line can fold in the faction name).
pub struct Line {
    pub speaker: String,
    pub title: String,
    pub body: String,
}

impl Line {
    fn new(speaker: &str, title: &str, body: &str) -> Line {
        Line {
            speaker: speaker.into(),
            title: title.into(),
            body: body.into(),
        }
    }
}

/// A faction's self-introduction, shown the first time the player's territory reaches it.
/// Always returns a line so **every** border contact prompts a greeting; named factions get a
/// written one, the rest a generic line (add specifics here as the campaign grows).
pub fn first_contact(faction: &str) -> Line {
    let (title, body) = match faction {
        // — Independent clans (Caesar's 54 BC submitters): the first-blood targets —
        "Cassi" => ("Independent clan", "We are the Cassi, who knelt to Caesar a hundred years past. We are few, queen of the Iceni — be swift."),
        "Bibroci" => ("Independent clan", "Bibroci, we are. We pay Rome's grain-tithe to keep our hearths warm. We have no stomach for your war."),
        "Ancalites" => ("Independent clan", "The Ancalites feast with Rome's tax-men. You will find us soft, daughter of the Iceni."),
        "Segontiaci" => ("Independent clan", "Segontiaci — the last of the kneelers. Take us, and all the east marches under your banner."),
        // — Kin: Boudica's own people (Team Red) —
        "Trinovantes" => ("Sworn ally", "The Trinovantes stand with you, Boudica. Rome seized our land for Camulodunum — we have not forgotten."),
        "Venta Icenorum" => ("Iceni capital", "Venta Icenorum, your own seat, my queen. The Iceni rally to your standard."),
        "Snettisham" => ("Iceni sanctuary", "Snettisham, keepers of the Iceni gold. Our torcs and our spears are yours."),
        "Thetford" => ("Iceni stronghold", "Thetford, hearth of the Iceni. Lead us against the eagle."),
        "Stonea" => ("Iceni fort", "Stonea, where we first defied Rome's disarmament. We rise again with you."),
        // — Rome's vassals (client tribes, Team Blue) —
        "Catuvellauni" => ("Client of Rome", "The Catuvellauni serve Rome now. We will not throw away what the eagle has given us."),
        "Atrebates" => ("Client of Rome", "Atrebates, friends of Rome under good king Cogidubnus. We want no part of your rebellion."),
        "Cantiaci" => ("Client of Rome", "Cantiaci of Kent, the gateway to Rome. We stand with the legions."),
        // — City bosses + Rome (Team Blue) —
        "Camulodunum" => ("Roman colony", "I am Camulodunum, where the Temple of Claudius stands. You would not dare, savage."),
        "Londinium" => ("Roman city", "Londinium, jewel of Roman trade. Turn back, Iceni, or burn with us."),
        "Verulamium" => ("Roman city", "Verulamium, loyal to Rome. Your rebellion ends at our walls."),
        "Rome" => ("The Empire", "I am Rome. I have swallowed greater kingdoms than yours, woman. Kneel, or be crushed."),
        // — Generic: any other faction still greets you —
        other => {
            return Line {
                speaker: other.into(),
                title: "A people of Britain".into(),
                body: format!("The {other} watch your rising, queen of the Iceni. This is not yet our fight."),
            };
        }
    };
    Line::new(faction, title, body)
}
