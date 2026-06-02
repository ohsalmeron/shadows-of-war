use leptos::prelude::*;

#[component]
pub fn SiteLayout(
    title: String,
    description: String,
    canonical: String,
    children: Children,
) -> impl IntoView {
    view! {
        <html lang="en">
            <head>
                <meta charset="UTF-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
                <meta name="description" content=description/>
                <meta name="theme-color" content="#101018"/>
                <title>{title.clone()}</title>
                <link rel="icon" href="/sow.svg" type="image/svg+xml"/>
                <link rel="canonical" href=canonical/>
                <style>{SITE_CSS}</style>
            </head>
            <body>
                <header class="site-header">
                    <a href="/" class="brand">"Shadows of War"</a>
                    <nav>
                        <a href="/play/">"Play"</a>
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
        max-width: 720px;
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
        padding: 2rem 0 3rem;
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
        margin: 0 auto 2rem;
    }
    .play-btn {
        display: inline-block;
        padding: 0.85rem 2.5rem;
        background: #3d6b8e;
        color: #fff;
        text-decoration: none;
        border-radius: 6px;
        font-weight: 600;
        font-size: 1.1rem;
    }
    .play-btn:hover { background: #4a7fa8; }
    h1, h2 { color: #fff; }
    ul { padding-left: 1.25rem; }
"#;
