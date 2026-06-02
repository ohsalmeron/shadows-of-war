use crate::game_manifest::{manifest_json, play_ready};
use crate::layout::SiteLayout;
use leptos::prelude::*;

const DESC: &str = "Command massive armies and conquer territories in Shadows of War, a free multiplayer grand strategy browser game. Play instantly—no download.";

#[component]
pub fn HomePage() -> impl IntoView {
    let ready = play_ready();
    let manifest = manifest_json();

    view! {
        <SiteLayout
            title="Shadows of War — Epic Multiplayer Grand Strategy".into()
            description=DESC.into()
            canonical="https://shadowsofwar.io/".into()
            og_title="Shadows of War - Epic Multiplayer Grand Strategy Game".into()
            game_manifest_json=if ready { manifest } else { String::new() }
        >
            <div class="hero">
                <h1>"Shadows of War"</h1>
                <p>
                    "Command massive armies, conquer territories, and dominate opponents in an "
                    "intense multiplayer grand strategy game. Play instantly in your browser."
                </p>
            </div>

            <div id="game-stage">
                <div id="game-play-overlay">
                    <button
                        type="button"
                        id="game-play-btn"
                        disabled=!ready
                    >
                        {if ready { "Play Now" } else { "Game build unavailable" }}
                    </button>
                </div>
            </div>

            <section>
                <h2>"About"</h2>
                <p>
                    "Command civilizations and tribes on world maps. "
                    "Single-player works offline; multiplayer uses our public server."
                </p>
            </section>
        </SiteLayout>
    }
}
