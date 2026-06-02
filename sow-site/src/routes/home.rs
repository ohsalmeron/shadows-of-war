use crate::layout::SiteLayout;
use leptos::prelude::*;

const DESC: &str = "Command massive armies and conquer territories in Shadows of War, a free multiplayer grand strategy browser game. Play instantly—no download.";

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <SiteLayout
            title="Shadows of War — Epic Multiplayer Grand Strategy".into()
            description=DESC.into()
            canonical="https://shadowsofwar.io/".into()
        >
            <div class="hero">
                <h1>"Shadows of War"</h1>
                <p>
                    "Command massive armies, conquer territories, and dominate opponents in an "
                    "intense multiplayer grand strategy game. Play instantly in your browser."
                </p>
                <a href="/play/" class="play-btn">"Play Now"</a>
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
