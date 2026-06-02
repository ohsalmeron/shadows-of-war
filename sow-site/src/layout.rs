use leptos::prelude::*;

#[component]
pub fn SiteLayout(
    title: String,
    description: String,
    canonical: String,
    #[prop(optional)] og_title: Option<String>,
    #[prop(default = String::new())]
    game_manifest_json: String,
    children: Children,
) -> impl IntoView {
    let og = og_title.unwrap_or_else(|| title.clone());
    let embed_game = !game_manifest_json.is_empty();
    let manifest = game_manifest_json;

    view! {
        <html lang="en">
            <head>
                <meta charset="UTF-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
                <meta name="description" content=description.clone()/>
                <meta name="keywords" content="strategy game, multiplayer strategy, io game, browser game, grand strategy, conquest, free to play"/>
                <meta name="theme-color" content="#101018"/>
                <title>{title.clone()}</title>
                <link rel="icon" href="/sow.svg" type="image/svg+xml"/>
                <link rel="canonical" href=canonical.clone()/>

                <meta prop:property="og:type" content="website"/>
                <meta prop:property="og:url" content=canonical.clone()/>
                <meta prop:property="og:title" content=og.clone()/>
                <meta prop:property="og:description" content=description.clone()/>

                <meta prop:property="twitter:card" content="summary_large_image"/>
                <meta prop:property="twitter:url" content=canonical.clone()/>
                <meta prop:property="twitter:title" content=og.clone()/>
                <meta prop:property="twitter:description" content=description.clone()/>

                <style>{SITE_CSS}</style>

                {embed_game.then(|| view! {
                    <script type="application/json" id="sow-game-manifest">{manifest}</script>
                    <script src="/boot.js" defer></script>
                })}
            </head>
            <body>
                <header class="site-header">
                    <a href="/" class="brand">"Shadows of War"</a>
                    <nav>
                        <a href="/#game-stage">"Play"</a>
                        <a href="/privacy">"Privacy"</a>
                        <a href="/terms">"Terms"</a>
                    </nav>
                </header>
                <main class="site-main">
                    {children()}
                </main>
                <footer class="site-footer">
                    <p>
                        "Shadows of War © Omar Hernandez Salmeron. "
                        "Based on "
                        <a href="https://openfront.io">"OpenFront"</a>
                        " — © OpenFront and Contributors. "
                        <a href="https://www.gnu.org/licenses/agpl-3.0.html">"AGPL-3.0-or-later"</a>
                        " · "
                        <a href="https://github.com/ohsalmeron/shadows-of-war">"Source"</a>
                    </p>
                </footer>
            </body>
        </html>
    }
}

const SITE_CSS: &str = r#"
    :root { color-scheme: dark; }
    body {
        font-family: system-ui, sans-serif;
        margin: 0;
        line-height: 1.6;
        color: #e0e0e0;
        background: #101018;
        min-height: 100vh;
        display: flex;
        flex-direction: column;
    }
    a { color: #7ec8e3; }
    .site-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 1rem 1.5rem;
        border-bottom: 1px solid #2a2a38;
        background: #0a0a12;
    }
    .brand {
        font-weight: 700;
        font-size: 1.1rem;
        text-decoration: none;
        color: #fff;
    }
    .site-header nav {
        display: flex;
        gap: 1.25rem;
    }
    .site-header nav a {
        text-decoration: none;
        color: #bdbdc8;
    }
    .site-header nav a:hover { color: #fff; }
    .site-main {
        flex: 1;
        max-width: 960px;
        margin: 0 auto;
        padding: 2rem 1.5rem;
        width: 100%;
        box-sizing: border-box;
    }
    .site-footer {
        padding: 1.5rem;
        border-top: 1px solid #2a2a38;
        font-size: 0.85rem;
        color: #888;
        text-align: center;
    }
    .hero {
        text-align: center;
        padding: 1rem 0 2rem;
    }
    .hero h1 {
        font-size: 2.25rem;
        color: #fff;
        margin-bottom: 0.75rem;
    }
    .hero p {
        font-size: 1.05rem;
        color: #b0b0bc;
        max-width: 540px;
        margin: 0 auto 1.5rem;
    }
    h1, h2 { color: #fff; }
    ul { padding-left: 1.25rem; }

    #game-stage {
        width: 100%;
        margin: 0 auto 2rem;
        position: relative;
        background: #0a0a0f;
        border-radius: 8px;
        overflow: hidden;
        border: 1px solid #2a2a38;
    }
    @media (min-aspect-ratio: 4/3) {
        #game-stage { aspect-ratio: 16 / 9; }
    }
    @media (min-aspect-ratio: 3/4) and (max-aspect-ratio: 4/3) {
        #game-stage { aspect-ratio: 1 / 1; }
    }
    @media (max-aspect-ratio: 3/4) {
        #game-stage { aspect-ratio: 9 / 16; }
    }
    #game-stage canvas {
        display: block;
        width: 100%;
        height: 100%;
    }
    #game-stage #version {
        position: absolute;
        bottom: 4%;
        right: 3%;
        color: rgba(189, 189, 189, 0.71);
        font-size: 0.5rem;
        pointer-events: none;
        z-index: 9999;
    }
    #game-play-overlay {
        position: absolute;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        background: rgba(10, 10, 15, 0.55);
        z-index: 2;
    }
    #game-play-btn {
        padding: 0.85rem 2.5rem;
        background: #3d6b8e;
        color: #fff;
        border: none;
        border-radius: 6px;
        font-weight: 600;
        font-size: 1.1rem;
        cursor: pointer;
        font-family: inherit;
    }
    #game-play-btn:hover { background: #4a7fa8; }
    #game-play-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    .game-stage--active #game-play-overlay { display: none; }

    #game-stage #web-loader {
        position: absolute;
        inset: 0;
        z-index: 10000;
        background-color: #0a0a0f;
        overflow: hidden;
    }
    #game-stage #web-loader .splash-bg {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        object-fit: cover;
        object-position: center;
        pointer-events: none;
    }
    #game-stage #web-loader .loader-bar-wrap {
        position: absolute;
        left: 50%;
        transform: translateX(-50%);
        pointer-events: none;
        overflow: visible;
    }
    #game-stage #web-loader .loader-bar-wrap .loader-text {
        top: 50%;
        left: 50%;
        z-index: 2;
    }
    #game-stage #web-loader .loader-bar-empty {
        display: block;
        width: 100%;
        height: 100%;
    }
    #game-stage #web-loader .loader-bar-fill {
        position: absolute;
        top: 0;
        left: 0;
        height: 100%;
        width: 0%;
        overflow: hidden;
        will-change: width;
    }
    #game-stage #web-loader .loader-bar-full {
        display: block;
        height: 100%;
        width: auto;
        max-width: none;
    }
    #game-stage #web-loader .loader-text {
        position: absolute;
        left: 50%;
        transform: translate(-50%, -50%);
        margin: 0;
        line-height: 1;
        font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
        font-weight: 600;
        color: #fff;
        text-shadow:
            0 2px 0 #000,
            0 4px 0 #000,
            -1px -1px 0 #000,
            1px -1px 0 #000,
            -1px 1px 0 #000,
            1px 1px 0 #000;
        white-space: nowrap;
        pointer-events: none;
    }
    @media (prefers-reduced-motion: reduce) {
        #game-stage #web-loader { transition: none; }
    }
"#;
