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
                    "Shadows of War is a derivative work based on "
                    <a href="https://openfront.io">"OpenFront"</a>
                    ", rebuilt in Rust with new leaders, visuals, and map tooling. "
                    "Single-player works offline; multiplayer connects to our public server."
                </p>
            </section>
        </SiteLayout>
    }
}
