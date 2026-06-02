use crate::layout::SiteLayout;
use leptos::prelude::*;

const DESC: &str = "Terms of service for Shadows of War: acceptable use, AGPL licensing, disclaimers, and links to our privacy policy.";

#[component]
pub fn TermsPage() -> impl IntoView {
    view! {
        <SiteLayout
            title="Terms of Service — Shadows of War".into()
            description=DESC.into()
            canonical="https://shadowsofwar.io/terms".into()
        >
            <h1>"Terms of Service"</h1>
            <p><strong>"Shadows of War"</strong>" — Copyright (c) Omar Hernandez Salmeron"</p>
            <p>"Last updated: May 2026"</p>

            <h2>"Acceptance"</h2>
            <p>
                "By playing Shadows of War on shadowsofwar.io, partner platforms, or native "
                "clients, you agree to these terms."
            </p>

            <h2>"The game"</h2>
            <p>
                "Shadows of War is free-to-play strategy software provided as-is. Multiplayer "
                "requires network access to our servers. We may modify, suspend, or discontinue "
                "services at any time."
            </p>

            <h2>"Conduct"</h2>
            <ul>
                <li>"Do not cheat, exploit bugs for unfair advantage, or harass other players."</li>
                <li>"Do not attempt to disrupt servers or reverse-engineer credentials."</li>
                <li>"Usernames and chat must not violate applicable law or platform rules."</li>
            </ul>

            <h2>"Intellectual property"</h2>
            <p>
                "Source is AGPL-3.0-or-later. Based on OpenFront — © OpenFront and Contributors. "
                "See in-game Credits and "
                <a href="https://github.com/ohsalmeron/shadows-of-war">"GitHub NOTICE"</a>
                "."
            </p>

            <h2>"Disclaimer"</h2>
            <p>
                "THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND. See the "
                "GNU Affero General Public License for full disclaimer language."
            </p>

            <h2>"Privacy"</h2>
            <p>
                "See our "
                <a href="/privacy">"Privacy Policy"</a>
                " for data handling."
            </p>

            <p><a href="/play/">"← Play game"</a></p>
        </SiteLayout>
    }
}
