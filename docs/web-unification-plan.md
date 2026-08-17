# Web Unification Plan — sow repo, single source of truth

Approved 2026-08-17. Execute P0→P9 non-stop. Rollback = `git revert` + `./sow p`.

## Target architecture

```
shadowsofwar.io/
  /            → website (sow-web/site) — SEO, canonical, sitemap
  /play/       → game shell — one URL, no duplication
  /maps/ /assets/  → game CDN (proxy 25566, unchanged)
  /ws/ /api/   → proxy (unchanged)

sow-web/
  shell/       → game shell (unchanged)
  site/        → website + legal/ (legal must NOT move: sow-i18n include_str!)

dist/
  web/         → full webroot (website + play/ + CDN content)  [was dist/play]
  crazygames/  → index.html + .br pair + sdk/ + sw.js + manifest + locales/ + svg only (~3.5MB, no zip)
  freebsd-bin/ releases/ .sow-state/ → pipeline internals (unchanged)
```

## Phases

- P0 Tree clean, release noted (0.1.2-7429ee43ffc4), this doc.
- P1 Migrate website into sow-web/site/: index.html, app.js, styles.css, data.js ONLY.
  Excluded as dead weight: wasm x4, v4_art (15MB, 0 refs), assets/cdn (dup → rewrite to /assets/cdn), assets/assets, assets/maps, assets/locales, assets/sdk, data.json (runtime uses data.js script tag).
  PLAY button → navigate /play/ (was simulated embed). SEO: canonical, OG, Twitter, JSON-LD VideoGame.
- P2 sow-dist/src/main.rs: dist_play→dist_web; package_self stages website at webroot root + robots.txt + sitemap.xml; delete admin copy block (was 521-529); package_cg whitelist rebuild (index at bundle root, SOW_ASSETS_URL restored, uncompressed-js filter fixed); verify_layout negative assertions.
- P3 sow-dist/src/prod.rs: require web/index.html + web/play/index.html; public verify adds /.
- P4 Delete dashboard: sow-server/src/main.rs route /admin/dashboard + admin_dashboard.html. Keep /admin/api/status (authorized debug endpoint).
- P5 nginx template: / → /index.html; ^~ /play/ SPA fallback; static /; delete /admin/ block; drop play./cdn. fossils from server_name.
- P6 rm -rf dist/site-dev (orphan, 0 refs).
- P7 cargo fmt + clippy -D warnings + test -p sow-dist; check sow-server.
- P8 ./sow p (single deploy, webroot + nginx together).
- P9 Runtime verify: / 200 website, /play/ 200, robots+sitemap 200, /maps+/assets 200, /admin/dashboard 404, CG bundle clean ~3.5MB, no 401s. Final gate: user plays.

## Manual steps (user, outside repo)
1. CrazyGames devportal: entry → index.html, upload folder (no zip).
2. Cloudflare: remove play./cdn. DNS fossils.
3. Archive/delete shadowsofwar.com repo after prod verified.
