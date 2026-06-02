use crate::layout::SiteLayout;
use leptos::prelude::*;

const DESC: &str = "Privacy policy for Shadows of War: gameplay data, partner platforms (CrazyGames, Poki), local storage, and AGPL source availability.";

#[component]
pub fn PrivacyPage() -> impl IntoView {
    view! {
        <SiteLayout
            title="Privacy Policy — Shadows of War".into()
            description=DESC.into()
            canonical="https://shadowsofwar.io/privacy".into()
        >
            <h1>"Privacy Policy"</h1>
            <p><strong>"Shadows of War"</strong>" — Copyright (c) Omar Hernandez Salmeron"</p>
            <p>"Last updated: May 2026"</p>

            <h2>"Overview"</h2>
            <p>
                "Shadows of War is a browser and native multiplayer strategy game. This policy "
                "describes what data we process when you play on shadowsofwar.io or partner "
                "platforms (CrazyGames, Poki)."
            </p>

            <h2>"Data we collect"</h2>
            <ul>
                <li>
                    <strong>"Gameplay:"</strong>
                    " Display name, clan tag, leader/civilization choices, and match inputs "
                    "required for lockstep multiplayer."
                </li>
                <li>
                    <strong>"Technical:"</strong>
                    " Client build version, WebSocket connection metadata, and basic server logs "
                    "for abuse prevention."
                </li>
                <li>
                    <strong>"Partner platforms:"</strong>
                    " When published on CrazyGames or Poki, their SDKs may collect analytics per "
                    "their respective policies. We do not sell personal data."
                </li>
            </ul>

            <h2>"Local storage"</h2>
            <p>
                "The web client may use browser local storage for settings and cached map assets. "
                "No third-party advertising trackers are embedded in the open-source client."
            </p>

            <h2>"Corresponding source"</h2>
            <p>
                "Server-side components are licensed under AGPL-3.0-or-later. Source code: "
                <a href="https://github.com/ohsalmeron/shadows-of-war">"github.com/ohsalmeron/shadows-of-war"</a>
                "."
            </p>

            <h2>"Contact"</h2>
            <p>
                "Privacy inquiries: security contact in "
                <a href="https://github.com/ohsalmeron/shadows-of-war/blob/main/SECURITY.md">"SECURITY.md"</a>
                "."
            </p>

            <p><a href="/play/">"← Play game"</a></p>
        </SiteLayout>
    }
}
