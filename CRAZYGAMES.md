
doc_content = """# CrazyGames Documentation — Complete Consolidated Reference

> Compiled from docs.crazygames.com | Last updated: 2026-06-20
> Source: https://docs.crazygames.com/

---

# TABLE OF CONTENTS

1. [Introduction](#1-introduction)
2. [Launching on CrazyGames](#2-launching-on-crazygames)
3. [Requirements](#3-requirements)
   - 3.1 Introduction to Requirements
   - 3.2 Technical Requirements
   - 3.3 Gameplay Requirements
   - 3.4 Advertisement Requirements
   - 3.5 Account Integration Requirements
   - 3.6 Multiplayer Requirements
   - 3.7 Game Covers
4. [SDK](#4-sdk)
   - 4.1 HTML5 v3 SDK
   - 4.2 HTML5 v2 SDK (Legacy)
   - 4.3 Unity SDK
   - 4.4 Godot SDK
   - 4.5 In-Game Purchases
   - 4.6 Banners
5. [Resources](#5-resources)
   - 5.1 Quality Guidelines
   - 5.2 Basic Launch Metrics Guide
   - 5.3 Unity Custom Build
   - 5.4 Unity Optimizer Package
   - 5.5 Mouse Control
   - 5.6 HTML5 Resources
   - 5.7 Unity Common Issues
   - 5.8 Sitelock
6. [Partners](#6-partners)
7. [FAQ & Contact](#7-faq--contact)
8. [Payouts](#8-payouts)

---

# 1. INTRODUCTION

Welcome to the documentation page for publishing a web game on CrazyGames. This page covers our game requirements and SDK documentation, while introducing you various resources and guidance to launch successful web-games. By publishing your game on CrazyGames, you can expect these benefits:

- Your games available on desktop and mobile devices
- Reach millions of gamers, many of them registered
- Earn revenue with ads (and in-game purchases for selected games only)
- Save game progress in the cloud easily
- Engage gamers with in-game friend invites
- Get statistics and feedback for your games
- Join an ever-growing community of passionate developers

---

# 2. LAUNCHING ON CRAZYGAMES

All game submissions are carefully reviewed by our QA team according to our technical and quality requirements. Please take the time to read our requirements section to ensure your game submission is accepted.

Games follow a two-stage launch process: **Basic Launch** and **Full Launch**. The goal of this process is to shorten the initial launch timeline and evaluate real-world performance before proceeding to a global release.

## Basic Launch

- Test your game on our platform with a limited audience for a temporary 2-week period.
- Requires Basic Implementation; no CrazyGames-specific integration and only Basic QA review.
- Monetization (video ads, banners, in-game purchases) is disabled.
- Proceed to Full Launch if metrics are good.

## Full Launch

- Your game is selected for global release.
- Requires Full Implementation of CrazyGames requirements, including a Full QA review.
- Monetization is enabled and you start receiving revenue share.
- During integration, the Basic version remains available to its initial limited audience.

## Progression Metrics

Progression to the Full Launch stage is based on key engagement metrics:
- Average playtime
- Conversion to gameplay
- Retention

These metrics will be benchmarked against other games on the platform. At the end of this 2-week period, you'll be notified about the next steps:

- If **all metrics meet or exceed** benchmarks: invited to update game for Full Launch.
- If **some metrics meet benchmarks**: may be invited to improve and request another Basic Launch.
- If **most metrics fall below benchmarks**: game can't proceed to Full Launch. Must submit as new game with significant improvements.

In some cases (multiplayer titles requiring larger audience, games already published elsewhere and specifically invited), you may bypass Basic Launch and proceed directly to Full Launch.

---

# 3. REQUIREMENTS

## 3.1 Introduction to Requirements

To be published on CrazyGames, your game must meet our requirements. We designed these standards to ensure all games on our platform are fun, unique, visually appealing, and properly integrated.

### Summary Table

| Category | Basic Implementation | Full Implementation |
|---|---|---|
| **Technical** | Initial download size ≤ 50MB; Total file size ≤ 250MB; File count ≤ 1500 | SDK & GameplayStart event |
| **Gameplay** | Basic visual QA checks; Adhere to PEGI12 | Full visual QA check; Land directly in gameplay |
| **Advertisement** | CrazyGames monetization disabled; No external ads | Ads through SDK, following guidelines; Works with AdBlock |
| **Account Integration** (when applicable) | No external login options | Progress linked to CrazyGames Account; Use CG username & avatar; Auto-login |
| **Multiplayer** (when applicable) | Full features optional | User room info; Invite link; Instant multiplayer; Keep rooms across rounds; DisableChat preference |
| **In-game Purchases** (Invite Only) | Not available | Use CrazyGames Xsolla account and userId |

### Monetization

The primary monetization mechanism is advertisement revenue share. Only ads served through our SDK are allowed.

Selected games are eligible for In-game Purchases (Full Implementation required, using Xsolla). Contact our team to apply.

### Insights & Analytics

Default metrics on Developer Dashboard:
- Players
- Average playtime
- Gameplay conversion
- Retention
- Revenue

For advanced analytics (level progression, drop-off points, user journey tracking), we recommend **ByteBrew** (free, simple to integrate).

**Warning:** If your game collects additional personal data beyond SDK events, add a Terms & Conditions and/or Privacy Policy notice to new players.

### Technical Support for SDK Integration

Once your games reach **50k plays** (combined), we can offer technical support with SDK integration.

### Quality Assurance Tool

On our Developer Portal you'll be able to preview your game. It allows you to:
- Run your game as it would on CrazyGames
- Check if your game meets our requirements
- Test all SDK features and get feedback

---

## 3.2 Technical Requirements

### File Size & Count Limits

- **Basic Implementation:** Maximum total file size of 250MB. File count limit of 1500 files.
- **Initial download size ≤ 50MB** (≤ 20MB to be eligible for mobile homepage).
  - When SDK is integrated: measured from start of loading to first `Gameplay start` event.
  - Without SDK: total file size used and should be ≤ 50MB (20MB for mobile homepage).
  - For externally hosted files: QA evaluates based on time to reach gameplay (≤ 20 seconds).
- Use only **relative paths**. **Never use absolute paths**.

### Device & Browser Compatibility

- Games must work on Chrome and Edge. Safari issues will disable the game on that browser.
- A significant segment uses Chromebook. Games disabled on Chromium OS if not smooth on 4GB RAM.
- Game supports mouse, keyboard, and touch if mobile is supported.
- Game should be playable in landscape mode on desktop. Vertical/portrait games allowed with black bars or background images.
- Rely on our system info for device-specific experience.

### Mobile Game Requirements

- Initial download size ≤ 20MB for mobile homepage eligibility.
- Configure supported orientation in submission (website handles rotation lock).
- Add this CSS to prevent double-tap/selection issues:
```css
-webkit-user-select: none;
-moz-user-select: none;
-ms-user-select: none;
user-select: none;
```
- Unity games disabled on iOS by default (memory issues). Evaluated and potentially enabled after sufficient plays.
- Mobile games should work inside the CrazyGames App (fullscreen, safe areas).
- Unity graphics quality managed by CrazyGames (DPR = 1 for iOS/low-memory Android).

### Resuming Audio After iOS Interrupts

On iOS, AudioContext enters interrupted state when backgrounded. To restore:
```javascript
document.addEventListener("touchend", () => {
    if (audioContext && audioContext.state === "suspended") {
        audioContext.resume();
    }
});
```

### SDK Integration

**Basic SDK Integration:**
- `Gameplay start` event triggered when player reaches game state (used to measure initial download size).
- Ads not allowed in Basic Launch.

**Full SDK Integration:**
- `Gameplay start/stop` events
- `Data` module for saving progress
- `User` module for account integration
- `Load start/stop` events (optional)

### Sitelock & Whitelisting

Implement sitelock to prevent game files from being stolen. If implementing, whitelist all CrazyGames domains.

### User Consent

If game collects additional personal data beyond SDK events, add Terms & Conditions and/or Privacy Policy notice. Make it a simple notice rather than a blocking pop-up.

---

## 3.3 Gameplay Requirements

### Basic Gameplay Requirements

- **Readable Content:** Text and images must be legible on devices with `devicePixelRatio:1`, responsive iframe sizes (16:9 ratio), and mobile screens. Key iframe sizes:
  - Desktop non-fullscreen: 907x510, 1216x684, 1077x606, 821x462 px
  - Desktop fullscreen: 1366x768, 1920x1080, 1536x864, 1280x720 px
  - Mobile: 800x450 px
  - Tablet: 1080x607 px
- **Consistent Physics:** Must perform consistently across different monitor refresh rates (144Hz, 165Hz, etc.)
- **Language Support:** English localization required. If translations included, use user's language based on `locale` from SDK system info, fallback to English.
- **Intuitive controls:** On different device types. Check restricted keys section.
- **Smooth Performance:** Load quickly, play seamlessly without errors or crashes.
- **Originality:** Game names, assets, and content should be original.
- **Fullscreen Functionality:** Automatically provided by CrazyGames. Custom in-game fullscreen buttons prohibited.
- **No Cross-Promotion:** No external/internal game/platform promotions. Exceptions:
  - Community links (Discord, dev website) allowed on menu only (not leading to playable web version)
  - Game Store links (Epic, Steam) on desktop main menu or end of demo only
  - Backlinks to CG home/category accepted but not promoted
  - Links to other games in same series
  - App Store links never allowed in-game (use Developer Portal metadata)
- **Suited for minors:** PEGI 12 compliant. CrazyGames audience is 13+.

### Full Gameplay Requirements

- Games should land new users in gameplay immediately.
- If not feasible, maximum of 1 click allowed.

---

## 3.4 Advertisement Requirements

**Warning:** During Basic Launch, ads are disabled. If you integrated Ads SDK, team will check game runs smoothly with ads disabled.

Only Ads requested through the CrazyGames SDK are allowed.

### Video Ads

- **Midgame ads:** Between levels or stages
- **Rewarded ads:** When giving a reward (CrazyGames provides fallback banners)

**Rules:**
- Video ads cannot interrupt gameplay. Show at logical points (level transition, map change, player died). Not on navigational buttons.
- Game should be paused during video ad. Disable buttons or show spinner blocking interaction.
- Handle unfilled ad calls correctly (`adError` event).
- Game should be muted during video ad. Only mute when ad actually starts playing.
- Request midgame ads at opportune moments without worrying about frequency. SDK handles max 1 every 3 minutes.

### Rewarded Ads

- Should be special opportunities, not expectations.
- Poorly designed levels requiring rewarded ads to complete are not acceptable.

**Placement and Frequency:**
- Don't offer too often. Use timer or hide button.
- Don't chain multiple ads for single reward.
- Don't promote too aggressively.
- Request button should not appear on active gameplay screen.

**Reward UI:**
- Button easily accessible in consistent location.
- Continue-without-watching button same size/font/color.
- Clear that reward is optional immediately.
- Clear that ad must be watched for reward (video icon).
- Provide alternative (e.g., buy with in-game coins).

**Callbacks:**
- When `adFinished`: make it clear player is rewarded (animation/notification).
- When `adError`: do NOT reward the player.

**Rewarded Ad Examples:**
- In-game store ads (monetize "purchase" mindset)
- Post-level reward doubling
- Out-of-lives (limited, not every death)
- Not allowed: combine midgame ad between levels with rewarded to keep playing current level.

### In-Game Banners

- Only on useful screens open for at least 5 seconds on average.
- Don't block game UI on any size (including mobile).
- Don't show during gameplay.
- Must be clearly distinguishable from game content.
- Maximum 2 banners per screen.
- Banners can impact performance.

### Adblockers

- Players with AdBlocker should play normally. Never block or penalize them.
- Can block certain features; show notice that feature is blocked due to AdBlocker.
- Don't use popups (interfere with fullscreen and CG adblock notices).
- Don't keep rewarded ads clickable but without effect.

---

## 3.5 Account Integration Requirements

Over **35 million** players have a CrazyGames account.

### Integration Scenarios

**Scenario 1 — Games without users:**
- Not expected to integrate User module.
- Can use Data module or APS system for progress save.

**Scenario 2 — Standard Integration:**
- Retrieve user object from User module for username/avatar.
- If result is `null`, user is not logged in; continue as guest.
- Use Data module for progress save.

**Scenario 3 — Games with in-game accounts, custom back-end:**
- Allow both guests and registered CG users to play as guests by default.
- Disable external login options (Facebook, Google, email).
- Full integration required when game proves successful.

**Scenario 4 — Full Integration for games with custom back-end:**
- New logged-in CG users auto-registered & logged in.
- Returning CG users auto-logged in.
- CG guests can play as guests.
- No external logout/login allowed.
- If importing/exporting accounts, responsible for transferring progress.

### Progress Save

1. **Preferably:** CrazyGames Data module (saves on CG account, auto-syncs guest data on login).
2. **If own back-end:** Use User module to link back-end data to CG account.
3. **Alternatively:** Automatic Progress Save (APS) system — not allowed for games with in-game purchases.

### In-Game Account Integration Logic

**At game launch:**
- Retrieve current user via `getUserToken()` → JWT Token → verify on server → get `userId`.
- Request current user every time game starts.

**Option 1: User not logged in (`userNotAuthenticated` error):**
- Always allow playing as Guest.
- Don't create in-game accounts for guests (or ensure linkable to CG account later).
- Can show "Login with CrazyGames" button (not main CTA).
- Don't trigger Auth prompt automatically.

**Option 2: User logged in (`userId` returned):**
- Check if `userId` exists in back-end.
- If known: update username/avatar if stored, fetch data, start playing.
- If unknown: auto-create game account using CG account. Link via `userId`.

**During gameplay:**
- Detect guest → logged-in via Auth Listener. Follow logged-in flow, refresh game if needed.
- Logout refreshes entire page (game restarts from beginning).

**Login Button:**
- Not main CTA, good placement: top right corner.
- Triggers Auth prompt method.
- Don't allow other login methods.

**Logout & Account Linking:**
- External logout/login not allowed.
- Optional Account Link Prompt for importing/exporting accounts.

---

## 3.6 Multiplayer Requirements

Games with online multiplayer have **2x higher long-term retention**.

### How It Works

- Users must have CG account and be logged in to use Friends feature.
- Users can send/receive friend requests.
- When user is in joinable location:
  - Friends can join the user
  - User can invite friends (friends get notification if online)
- Game module in SDK supports this.
- Games supporting Friends featured on dedicated Multiplayer landing page.

### Requirements for "Online with Friends"

**Implement multiplayer flows:**
- **Sharing room & status:** Pass room info through SDK. Use `updateRoom()` with `room`, `isJoinable`, `inviteParams`.
- **Invite Link:** Allow copying direct invite links within game.
- **Instant Multiplayer:** First player in party placed directly into new private room with default settings. When `IsInstantMultiplayer` flag is `true`, launch directly into multiplayer mode from CG UI.
- **Round-based games:** After match, players should continue with same group without navigating back through CG UI.

**Additional requirements:**
- Submit lobby sizes when uploading.
- Display CrazyGames usernames in-game.
- If chat: disable based on settings, implement profanity filter or AI moderation (Lasso).

**Guidelines (recommended):**
- Players can join existing room at all times, spectator mode if round ongoing.
- Implement Room join listener for smoother UX without page reload.
- Use User module to get friends list for UX improvements.

**Multiplayer in Basic Launch:**
- Games requiring large audience may skip Basic Launch.
- Games with single-player component require Basic Launch.
- QA team decides which flow applies.

---

## 3.7 Game Covers

You must add 3 cover images (landscape, portrait, square) and upload preview videos when submitting your game.

---

# 4. SDK

## 4.1 HTML5 v3 SDK

### Requirements

When integrating, follow our requirements for technical, gameplay, ads, and account integration.

### Installation

```html
<script src="https://sdk.crazygames.com/crazygames-sdk-v3.js"></script>
```

### Manual Initialization

```javascript
await window.CrazyGames.SDK.init();
```

Important to `await` initialization — happens asynchronously. Recommend doing this on loading screen before game starts.

### Promises

The SDK relies on promises and doesn't accept `callback` parameter. Use `await` or `.then(...).catch(...)`.

```javascript
// await example
try {
    const user = await window.CrazyGames.SDK.user.getUser();
    console.log(user);
} catch (e) {
    console.log("Get user error: ", e);
}

// .then .catch example
window.CrazyGames.SDK.user
    .getUser()
    .then((user) => console.log(user))
    .catch((e) => console.log("Get user error: ", e));
```

### Development Environments

- `localhost` / `127.0.0.1`: `local` environment. Ads not available (overlay text shown). Console output for events.
  - Can enforce `local` with `?useLocalSdk=true` query parameter.
- `CrazyGames` domains: `crazygames` environment (full functionality).
- Other domains: `disabled` environment. All SDK calls throw errors.

```javascript
window.CrazyGames.SDK.environment; // 'local', 'crazygames', or 'disabled'
window.CrazyGames.SDK.isQaTool;    // check if running in QA Tool
```

### Modules

- `ad` — display video ads, detect adblockers
- `banner` — display banners
- `game` — various game events
- `user` — interact with currently logged-in user
- `data` — store user data that persists across devices (new in v3)

Access: `window.CrazyGames.SDK.[moduleName]`

### Migration from v2

- Manual initialization required now.
- Some async get methods are now simple variables:
  - `window.CrazyGames.SDK.environment`
  - `window.CrazyGames.SDK.user.isUserAccountAvailable`
  - `window.CrazyGames.SDK.user.systemInfo`
- Better error handling: `{code: 'userAlreadySignedIn', message: 'The user is already signed in'}`

### Data Module

Same API as localStorage:
```javascript
clear(): void;
getItem(key: string): string | null;
removeItem(key: string): void;
setItem(key: string, value: string): void;
```

```javascript
window.CrazyGames.SDK.data.setItem("gold", 100);
```

- Guest users: data stored in localStorage. Auto-synced to cloud on login.
- 1MB data limit. Warnings in console if approaching.
- Data saving debounced with 1 second (up to 30 seconds in some cases).
- Works in QA Tool but doesn't sync data (same data regardless of selected user).

### HTML5 v3 Game Module

```javascript
// Gameplay tracking
await window.CrazyGames.SDK.game.gameplayStart();
await window.CrazyGames.SDK.game.gameplayStop();

// Happy time (achievements, boss beaten, high score)
await window.CrazyGames.SDK.game.happyTime();

// Invite link
const inviteLink = await window.CrazyGames.SDK.game.inviteLink({ roomId: "1234" });

// Get invite link parameters
const roomId = await window.CrazyGames.SDK.game.getInviteLinkParameter("roomId");

// Invite button
await window.CrazyGames.SDK.game.showInviteButton({ roomId: "1234" });
await window.CrazyGames.SDK.game.hideInviteButton();

// Update room (multiplayer)
await window.CrazyGames.SDK.game.updateRoom({
    roomId: "room1",
    isJoinable: true,
    inviteParams: { mode: "team" }
});

// Join room listener
window.CrazyGames.SDK.game.addJoinRoomListener((roomInfo) => {
    console.log(roomInfo);
});

// Load start/stop (optional)
await window.CrazyGames.SDK.game.loadStart();
await window.CrazyGames.SDK.game.loadStop();
```

### HTML5 v3 Ad Module

```javascript
// Midgame ad
await window.CrazyGames.SDK.ad.requestAd("midgame", {
    adStarted: () => { /* mute audio, pause game */ },
    adError: (error) => { /* resume game */ },
    adFinished: () => { /* unmute audio, resume game */ }
});

// Rewarded ad
await window.CrazyGames.SDK.ad.requestAd("rewarded", {
    adStarted: () => { /* mute audio, pause game */ },
    adError: (error) => { /* don't reward */ },
    adFinished: () => { /* unmute audio, reward player */ }
});

// Check adblock
const hasAdblock = await window.CrazyGames.SDK.ad.hasAdblock();
```

### HTML5 v3 Banner Module

```javascript
await window.CrazyGames.SDK.banner.requestBanners([
    {
        id: "main-menu-banner-1",
        width: 300,
        height: 250,
        x: 0,
        y: 0,
    },
    {
        id: "main-menu-banner-2",
        width: 300,
        height: 250,
        x: 922 - 300,
        y: 0,
    },
]);

// Refresh banners
await window.CrazyGames.SDK.banner.refreshBanners();
```

### HTML5 v3 User Module

```javascript
// Check availability
const isAvailable = window.CrazyGames.SDK.user.isUserAccountAvailable;

// Get current user
const user = await window.CrazyGames.SDK.user.getUser();
// Returns: { username, profilePictureUrl } or null

// System info
const systemInfo = window.CrazyGames.SDK.user.systemInfo;
// { countryCode, browser: {name, version}, os: {name, version}, device: {type}, locale }

// Auth prompt
const user = await window.CrazyGames.SDK.user.showAuthPrompt();

// Get user token (JWT)
const token = await window.CrazyGames.SDK.user.getUserToken();

// Auth listener
window.CrazyGames.SDK.user.addAuthListener((user) => {
    console.log("User logged in:", user);
});

// Account link prompt
const answer = await window.CrazyGames.SDK.user.showAccountLinkPrompt();
```

### HTML5 v3 Analytics Module

```javascript
// Track order (Xsolla)
await window.CrazyGames.SDK.analytics.trackOrder("xsolla", orderData);
```

---

## 4.2 HTML5 v2 SDK (Legacy)

**Info:** We recommend migrating to the new HTML5 v3 SDK, which contains more features.

### Installation

```html
<script src="https://sdk.crazygames.com/crazygames-sdk-v2.js"></script>
```

The SDK doesn't need to be initialized before being used (initialization done internally).

### Development Environments

Same as v3 (`local`, `crazygames`, `disabled`).

```javascript
const callback = (_error, environment) => {
    console.log(environment); // 'local', 'crazygames' or 'disabled'
};
window.CrazyGames.SDK.getEnvironment(callback);

// Or with await
const environment = await window.CrazyGames.SDK.getEnvironment();
```

---

## 4.3 Unity SDK

### Download & Setup

- Delete folders `CrazySDK` and `CrazyOptimizer` (if present) before importing new SDK.
- SDK found in `CrazyGames` namespace.

```csharp
using CrazyGames;
```

Before calling any SDK functionality, ensure `CrazySDK.IsAvailable` is true. Available on CrazyGames, in Editor, and on localhost.

### Initialization

```csharp
CrazySDK.Init(() => { /* initialization finished callback */ });
```

Don't call SDK methods until callback is called. Avoid initializing in `[RuntimeInitializeOnLoadMethod]` methods (conflicts with SDK's internal initialization).

Demo loading scene available at `CrazySDK/Demo/LoadingScene`. Drag into build scenes as first scene, set `nextSceneName`.

### Testing Locally

- Functional in Unity Editor and local browser build.
- Must run on `localhost` or `127.0.0.1` (otherwise sitelock blocks).
- User token and Xsolla token not available locally — test in QA Tool for these.

### Sitelock

Automatic on game start. Allows game only on CrazyGames.com and affiliated sites.

Whitelist your domain in `CrazySDK/Resources/CrazyGamesSettings`.

### Addressables/AssetBundles/Streaming Assets

- Recommended for reducing initial load size.
- External asset loading supported for Unity 2020+.
- Folder must be named `StreamingAssets`.
- Use `Application.streamingAssetsPath` for correct URLs.

### QA Tool

```csharp
CrazySDK.IsQaTool;
```

### Unity Game Module

```csharp
// Instant join (multiplayer)
bool isInstantJoin = CrazySDK.Game.IsInstantJoin;

// Gameplay tracking
CrazySDK.Game.GameplayStart();
CrazySDK.Game.GameplayStop();

// Happy time
CrazySDK.Game.HappyTime();

// Invite link
var parameters = new Dictionary<string, string>();
parameters.Add("roomId", "1234");
var inviteLink = CrazySDK.Game.InviteLink(parameters);

// Copy to clipboard
CrazySDK.Game.CopyToClipboard(inviteLink);

// Get invite parameter
var roomId = CrazySDK.Game.GetInviteLinkParameter("roomId");

// Invite button
var inviteLink = CrazySDK.Game.ShowInviteButton(parameters);
CrazySDK.Game.HideInviteButton();
```

### Unity User Module

```csharp
// Check availability
var isAvailable = CrazySDK.User.IsUserAccountAvailable;

// Get current user
CrazySDK.User.GetUser(user => {
    if (user != null) {
        Debug.Log("User: " + user.username + ", " + user.profilePictureUrl);
    } else {
        Debug.Log("User not logged in");
    }
});

// System info
var systemInfo = CrazySDK.User.SystemInfo;
Debug.Log(systemInfo.countryCode);
Debug.Log(systemInfo.browser.name);
Debug.Log(systemInfo.browser.version);
Debug.Log(systemInfo.os.name);
Debug.Log(systemInfo.os.version);
Debug.Log(systemInfo.device.type); // "desktop", "tablet", "mobile"

// Auth prompt
CrazySDK.User.ShowAuthPrompt((error, user) => {
    if (error != null) { Debug.LogError("Auth prompt error: " + error); return; }
    Debug.Log("Auth prompt user: " + user);
});

// Get user token
CrazySDK.User.GetUserToken((error, token) => {
    if (error != null) { Debug.LogError("Token error: " + error); return; }
    Debug.Log("Token: " + token);
});

// Auth listener
Action<PortalUser> lst = (user) => { Debug.Log("Auth listener: " + user); };
CrazySDK.User.AddAuthListener(lst);
CrazySDK.User.RemoveAuthListener(lst);

// Sync game data
CrazySDK.User.SyncUnityGameData();

// Account link prompt
CrazySDK.User.ShowAccountLinkPrompt((error, answer) => {
    if (error != null) { Debug.LogError("Link prompt error: " + error); return; }
    Debug.Log("Account link answer: " + answer);
});
```

### Unity Ad Module

```csharp
// Midgame ad
CrazySDK.Ad.RequestAd(CrazyAdType.Midgame,
    () => { /* adStarted - mute audio, pause game */ },
    () => { /* adError - resume game */ },
    () => { /* adFinished - unmute audio, resume game */ });

// Rewarded ad
CrazySDK.Ad.RequestAd(CrazyAdType.Rewarded,
    () => { /* adStarted */ },
    () => { /* adError - don't reward */ },
    () => { /* adFinished - reward player */ });
```

### Unity Banner Module

```csharp
// Request banners
CrazySDK.Banner.RequestBanner(
    bannerId: "menu-banner",
    width: 300,
    height: 250,
    position: CrazyBannerPosition.CenterMiddle,
    () => { Debug.Log("Banner loaded"); },
    (error) => { Debug.Log("Banner error: " + error); });

// Hide banner
CrazySDK.Banner.HideBanner("menu-banner");
```

### Unity Data Module

```csharp
// Same API as PlayerPrefs
CrazySDK.Data.SetItem("key", "value");
string value = CrazySDK.Data.GetItem("key");
CrazySDK.Data.RemoveItem("key");
CrazySDK.Data.Clear();
```

---

## 4.4 Godot SDK

### Installation

1. Create new project or add SDK to existing.
2. Under Project/Export, add HTML5 export preset named `CrazyGames`.
3. In Options tab, add to Head Include:
```html
<script src="https://sdk.crazygames.com/crazygames-sdk-v2.js"></script>
```
4. In Features tab, add custom feature: `crazygames`.
5. Create `CrazySDK.gd` extending `Node`, add to Autoload.
6. In `_ready()`, check `OS.has_feature("crazygames")`.
7. Access SDK via JavaScript singleton:
```gdscript
var window = JavaScript.get_interface("window")
SDK = window.CrazyGames.SDK
```

### Development

Export using CrazyGames preset, rename HTML to `index.html`, upload to QA Tool.
For local testing with HTML5 debug button, use:
```html
<script crossorigin="anonymous" src="https://sdk.crazygames.com/crazygames-sdk-v2.js"></script>
```

### Modules

- `ad` — display video ads, detect adblockers
- `banner` — display banners
- `game` — various game events

Access: `SDK.[moduleName]`

### Godot 4

Can be used with single-threaded build. Be careful of audio issues and different callback syntax:
```gdscript
# Godot 3
var adStartedCallback = JavaScript.create_callback(self, "adStarted")
# Godot 4
var adStartedCallback = JavaScript.create_callback(adStarted)
```

---

## 4.5 In-Game Purchases

**Invite only feature.** Contact team if interested.

In-game purchases available only for signed-in users. Guest users cannot purchase.

### Getting Started

**Standard (linked to CrazyGames user accounts):**
1. Create game on developer portal, contact team for token credentials.
2. Use `GetXsollaUserToken()` from SDK → generates custom Xsolla token.
3. Purchases linked to CrazyGames account automatically.

**Custom (linked to in-game accounts):**
1. Generate Xsolla credentials in CrazyGames Xsolla project dashboard.
2. Use Xsolla SDK with generated credentials.
3. Orders must reference CrazyGames `userId`.

### Registering Orders

Use Xsolla Shop Builder API.

**Warning:** If not using `GetXsollaUserToken()`, pass CrazyGames `userId` in orders (main identifier or in `custom_parameters`).

### Testing

- `GetXsollaUserToken()` only works on CrazyGames.com.
- Preview via Developer Portal.
- Test with sandbox orders (fake money).
- Disable sandbox before submission.

### Order Tracking

Track through SDK when order completed (`done` status):

```csharp
// Unity
XsollaCatalog.Purchase(skuField.text, orderStatus =>
{
    CrazySDK.Analytics.TrackOrder(PaymentProvider.Xsolla, orderStatus);
}, error => { Debug.LogError($"Failed: {error.errorMessage}"); });
```

```javascript
// HTML5
window.CrazyGames.SDK.analytics.trackOrder("xsolla", order);
```

Order statuses: `new`, `done`, `canceled`. Track all three for future use.

---

## 4.6 Banners

### HTML5 v3 Banners

```javascript
await window.CrazyGames.SDK.banner.requestBanners([
    { id: "banner-1", width: 300, height: 250, x: 0, y: 0 },
    { id: "banner-2", width: 728, height: 90, x: 100, y: 0 },
]);

await window.CrazyGames.SDK.banner.refreshBanners();
```

All 5 parameters required per banner: `id`, `x`, `y`, `width`, `height`.

### Unity Banners

```csharp
CrazySDK.Banner.RequestBanner(
    bannerId: "menu-banner",
    width: 300,
    height: 250,
    position: CrazyBannerPosition.CenterMiddle,
    onSuccess: () => { },
    onError: (error) => { });

CrazySDK.Banner.HideBanner("menu-banner");
```

### Godot Banners

Use built-in `CrazyBanner` control from addon (`res://addons/crazygames/Utils/crazy_banner.tscn`).

Supported sizes:
- `LEADERBOARD_728x90`
- `MEDIUM_300x250`
- `MOBILE_320x50`
- `MAIN_BANNER_468x60`
- `LARGE_MOBILE_320x100`

After showing/hiding, refresh visible overlays:
```gdscript
CrazyGames.Ad.refresh_banners()
```

---

# 5. RESOURCES

## 5.1 Quality Guidelines

Inspired by Facebook games. Optional but strongly recommended based on audience insights.

## 5.2 Basic Launch Metrics Guide

### Average Play Time
- **What:** Average time a player spends in a single session.
- **Why:** Longer sessions = hooked on core loop.
- **Benchmark:** Successful titles often see **10+ minutes**.
- **How to improve:** Rewarding loop, clear goals, paced content, fair difficulty curve.

### Day 1 Retention
- **What:** Percentage of players who return the day after first session.
- **Why:** Proves game is memorable.
- **Benchmark:** Strong games achieve **10-15%**.
- **How to improve:** Meaningful progression, daily hooks, save progress, polish.

### Conversion
- **What:** Percentage of players who play for at least one minute after clicking Play.
- **Why:** Low conversion = slow load times or confusing onboarding.
- **Benchmark:** Top titles convert **80%+**, load under **10 seconds**, build size below **20MB**.
- **How to improve:** Keep build small, dynamic loading, get to gameplay fast.

## 5.3 Unity Custom Build

Unity exposes many build settings affecting web performance. Custom build tool automatically applies right settings, generates device-optimized variants, and shows what's included.

### How to Create

- Latest Unity SDK adds `CrazySDK` top menu → `CrazyGames Build`.
- **Development build:** Quick local testing.
- **Release build:** Multiple builds for mobile/desktop optimization. Upload all files from `Builds/CrazyGamesRelease`.
- Release build applies **Disk Size With LTO** flag (builds take longer).

### Analyzer

Provides insights about assets and code in build. Shows:
- Packaged assets count
- Code size
- Texture size
- Audio size
- Assets in Resources folders
- Mipmap textures (with fix suggestions)

## 5.4 Unity Optimizer Package (Deprecated)

Deprecated in favor of custom CrazyGames builds. Still available on GitHub.

Minimum versions: C# 6.0, Unity 2019.

Integrated in SDK by default. Accessible in `Tools > WebGL Optimizer`.

**Features:**
- Export optimizations checklist
- Texture optimizations overview with tips
- Build logs analyzer (parses Editor.log)

## 5.5 Mouse Control

On desktop, many users play outside fullscreen. Avoid accidentally leaving game by clicking outside frame.

### Game Types

**First-person games:**
- Lock mouse in center during gameplay
- Keyboard shortcut to unlock (Escape, Tab)
- Example: Bloxd.io

**Top-view games (mouse gesture movement):**
- Lock & confine mouse to game area
- Show custom pointer or joystick method
- Keyboard shortcut to unlock
- UI buttons clickable or keyboard shortcut indicated
- Additional functionalities (drag-and-drop) managed from game
- Optionally add WASD/Arrow keys
- Examples: Little Big Fighters, GunMaster.io

**Other games (clickers, bubble shooter, etc.):**
- Limited mouse movement, no mouse confinement required.

### HTML5: Pointer Lock API

### Unity Example

```csharp
public Image customMouse;
public RectTransform canvas;
public float cursorSpeed = 20f;
private bool isLocked = false;

void Start()
{
   isLocked = true;
   customMouse.enabled = false;
   Cursor.lockState = CursorLockMode.Locked;
}

void Update()
{
   float mouseX = Input.GetAxis("Mouse X") * cursorSpeed;
   float mouseY = Input.GetAxis("Mouse Y") * cursorSpeed;
   Vector2 currentPosition = customMouse.rectTransform.localPosition;
   currentPosition.x += mouseX;
   currentPosition.y += mouseY;
   currentPosition.x = Mathf.Clamp(currentPosition.x, canvas.rect.min.x, canvas.rect.max.x);
   currentPosition.y = Mathf.Clamp(currentPosition.y, canvas.rect.min.y, canvas.rect.max.y);
   customMouse.rectTransform.localPosition = currentPosition;

   if (Input.anyKeyDown) {
      Cursor.lockState = CursorLockMode.Locked;
      if (isLocked) { customMouse.enabled = true; isLocked = false; }
   }

   if (Application.isFocused == false)
   {
      Cursor.lockState = CursorLockMode.None;
      if (!isLocked) { customMouse.enabled = false; isLocked = true; }
    }
}
```

**Warning:** Fake cursor won't click Unity UI buttons — implement additional logic.

## 5.6 HTML5 Resources

Reorganized into dedicated pages:
- Introduction
- Sitelock
- Common fixes

## 5.7 Unity Common Issues

### Cursor Locking

Don't lock cursor at startup before player interaction. Delay until after user action:
- Display overlay: "Please click to lock the cursor"
- Or require "Play" button click

### MacOS Rendering Issues (URP)

Potential fix: disable Depth Priming Mode in Universal Renderer Data.

## 5.8 Sitelock

### HTML5 Games

Check if running on `crazygames.*` domains. Example valid domain:
`https://cubes-2048-io.game-files.crazygames.com/cubes-2048-io/13/index.html`

```javascript
function isCrazyGames() {
    const hostname = window.location.hostname;
    const parts = hostname.split(".");
    const idx = parts.indexOf("crazygames");
    return idx !== -1 && idx >= parts.length - 3;
}
```

If check fails, show "Available only on CrazyGames" or blank screen.

Obfuscate relevant code with tools like obfuscator.io.

### Iframe Games

Configure CSP header: `Content-Security-Policy: frame-ancestors [...]`

Whitelist all CrazyGames domains:
```
// General
*.crazygames.com
crazygames.*

// Exhaustive list
www.crazygames.com
de.crazygames.com, it.crazygames.com, vn.crazygames.com, gr.crazygames.com, ar.crazygames.com, th.crazygames.com
www.crazygames.fr, www.crazygames.co.id, www.crazygames.cz, www.crazygames.dk, www.crazygames.hu, www.crazygames.nl, www.crazygames.no, www.crazygames.pl, www.crazygames.com.br, www.crazygames.ro, www.crazygames.fi, www.crazygames.se, www.crazygames.ru, www.crazygames.com.ua, www.crazygames.at, www.crazygames.jp, www.crazygames.pt, www.crazygames.vn, www.crazygames.com.vn, www.crazygames.co.kr
games.crazygames.com

// Deprecated (no longer need whitelisting)
www.1001juegos.com, tr.crazygames.com
```

---

# 6. PARTNERS

### Photon Backend
Multiplayer backend solution.

### Xsolla Payments
Payment provider for in-game purchases.

### ByteBrew Analytics
Free analytics tool for level progression, drop-off points, user journey tracking.

### Lasso Moderation
AI-based moderation for chat and user-generated content. Eligible for referral bonus when integrating.

---

# 7. FAQ & CONTACT

## Publishing Your Games

### How do you decide which games to launch?
Initial QA Check → Basic Launch (soft launch, 2 weeks) → Full Launch (if metrics good).

### Why are ads disabled during Basic Launch?
- Maximize player experience in early testing
- Ensure engagement metrics not biased by ad interruptions
- Focus on measuring organic retention and interest

### Can I publish a demo version?
Yes, as long as enough high-quality content.

### Can I test before publishing?
Yes, preview environment via Developer Portal.

### Do I still own the rights?
Yes, 100%. (Article 3.2 of Terms & Conditions)

### Which technologies accepted?
Unity, Godot, Phaser, Construct, Pixi.js, BabylonJS, PlayCanvas, GameMaker, and more.

### Can I upload a mobile/desktop game?
Platform attracts both mobile and desktop users. Mobile version not mandatory.

### What do I need to upload?
Game files, cover images, and videos.

### Can I use CrazyGames to promote mobile/Steam game?
Yes, browser is ideal for trying new concepts and discovering audience.

### Can I upload a previously published game?
Yes, encouraged. CrazyGames has 40 million users.

### Will you iframe my game from another portal?
No, not from competing browser game portals. Upload on developer portal instead.

### Will you iframe my game on my own domain?
Yes, considered regular submission. Revenue only if SDK correctly integrated.

### What dimensions should my game be?
See gameplay requirements page.

### Can I implement chat?
Yes, but monitor chat and implement profanity filter. SDK offers preference to disable chat.

### Do I need to add a CrazyGames logo?
Not required but appreciated. Pick from assets page.

### How do I know if accepted?
Via email.

### Can I submit a rejected game again?
Only if improved and complies with requirements.

### Does CrazyGames accept portrait/mobile-only games?
Yes, vertical/portrait games allowed. Display black bars or background on desktop.

### How long for update to go live?
Normally within the day (working days/hours). Reflected after cache refresh.

### Which countries accepted?
All countries and nationalities, no restriction.

## Managing Games After Publication

### Can I upload to other portals?
Yes, unless commercial agreement states otherwise.

### Can I see traffic?
Yes, dashboard in developer portal.

### Do I receive player feedback?
Yes, negative vote prompts feedback. Manage email preference.

### Can I update my game?
Yes, through developer account. Replace files and submit for approval.

### How is ranking calculated?
Based on device, country, OS. Individual personalization based on previous plays. Factors: play counts, average playtime, retention, conversion, votes. No pay-to-feature. New games get a boost.

### Will my game appear on homepage?
Eligible for new carousel after reaching specific player count. Stays based on popularity and engagement.

## Earning Money

### Will I earn money?
Yes, if:
- No branding from another portal
- No external advertisements
- Original content
- CrazyGames SDK integrated

### How much will I earn?
Depends on popularity, engagement rate, advertiser interest.

### How will I be paid?
Monthly with minimum 100 euros. Carried over if not reached. Wire transfer or PayPal.

## Technical Questions

### Does CrazyGames provide a CDN?
Yes, all uploaded files hosted and distributed via CDN. No cache invalidation needed.

### Can I use StreamingAssets/Addressables with Unity?
Yes, recommended to decrease initial download size. Just drag Build and StreamingAssets folder to upload area.

### What browsers should be supported?
At least Chrome and Edge. Also recommend Safari and Firefox.

### Do I need to learn coding?
Knowing how to program helps. Unity, Phaser require coding. GDevelop allows no-code.

---

# 8. PAYOUTS

CrazyGames uses **Tipalti**, a global payment automation platform, for monthly developer payouts.

Complete onboarding via Developer Portal early to prevent delays.

## Getting Started with Payment Setup

1. Log into Developer Portal
2. Navigate to Account → Billing
3. Complete Steps 1–4
4. Double-check all entries

## Payment Methods

- Wire Transfer
- Direct Deposit / ACH
- eCheck
- PayPal

Availability depends on country and local regulations. Each method may include fees and minimum thresholds.

Can temporarily hold payments. Funds released when updating to eligible option.

**Minimum:** €100 in earnings before issuing payout.

## Payment Timeline & Thresholds

Monthly payments, or once unpaid earnings reach €100.

NET 60 terms, but typically processed by 10th of following month (NET 10 in practice).

**Example 1:** €100 in January → invoice early February → payment mid-February.
**Example 2:** €30 in January + €70 in February → invoices early Feb/March → both paid mid-March.

Actual receipt depends on method, bank processing, country factors.

## Frequently Asked Questions

### Why can't I submit without completing payment setup?
Need details for self-billing invoices. Can complete without payment details by selecting "hold payments".

### Who to contact if stuck during payment setup?
Tipalti provides in-portal help. Email finance@crazygames.com for assistance.

### What is Tipalti?
Automates payments, supports international payout methods.

### How to complete registration?
Login → Account > Billing → complete all sections (Address, Tax, Payment).

### My address won't validate?
Click Next. If issues detected, scroll to top and accept suggested format.

### Can I choose different currency?
Yes, certain methods allow. FX fees apply for conversion.

### Business registered in one country, bank in another?
In step 1, click "if you want to be paid in a different country" at bottom of address form.

### Which tax form?
Tipalti wizard guides based on location and entity type.

### Why is PayPal not shown?
In step 1, set Payment Country to "United States". PayPal processed in USD. Select USD as currency to avoid FX fees.

### Tipalti doesn't process to my country?
Contact finance@crazygames.com. Can still receive monthly invoices with "Hold Payments".

### How to update address/bank info after registration?
Login → Account > Billing. Some changes may invalidate tax/bank info.

### How to pause payments?
Select "Hold Payments" under Account > Billing > Info (Step 2). Takes effect immediately.

### Why did I receive less than expected?
Bank may deduct intermediary fees or FX charges. CrazyGames has no control. Contact bank for breakdown.

### Can I bundle payments to lower costs?
Yes, use "Hold Payments" to accumulate, then update method for single payout.

### Made revenue but no invoice visible?
Wait a few business days after month-end. Invoicing threshold of 10 EUR — amounts below transferred to next month.

### Payment "in process" for long time?
Often means missing/invalid info. Check email for required updates. If locked, contact finance@crazygames.com.

### Payment was rejected?
Check rejection notes under Billing > History. Confirm info is accurate. System auto-reprocesses next cycle.

---

# APPENDIX: User Token Verification (TypeScript Example)

```typescript
import * as jwt from "jsonwebtoken";
import axios from "axios";

export interface CrazyTokenPayload {
    userId: string;
    gameId: string;
    username: string;
    profilePictureUrl: string;
}

export const DecodeCGToken = async (
    token: string
): Promise<CrazyTokenPayload> => {
    let key = "";

    try {
        const resp = await axios.get(
            "https://sdk.crazygames.com/publicKey.json"
        );
        key = resp.data["publicKey"];
    } catch (e) {
        console.error("Failed to fetch CrazyGames public key", e);
    }

    if (!key) {
        throw new Error("Key is empty when decoding CrazyGames token");
    }

    const payload = jwt.verify(token, key, { algorithms: ["RS256"] });
    return payload as CrazyTokenPayload;
};
```

Token payload:
```json
{
    "userId": "UOuZBKgjwpY9k4TSBB2NPugbsHD3",
    "gameId": "20267",
    "username": "RustyCake.ZU9H",
    "profilePictureUrl": "https://images.crazygames.com/userportal/avatars/16.png",
    "iat": 1670328680,
    "exp": 1670332280
}
```

- Token lifetime: 1 hour
- Don't store token; always call method when needed
- Fetch public key every time (or cache and re-fetch on failure)

---

*End of CrazyGames Documentation Consolidated Reference*
"""

# Save the file
with open('/mnt/agents/output/crazygames-documentation.md', 'w', encoding='utf-8') as f:
    f.write(doc_content)

print(f"File saved successfully!")
print(f"Total characters: {len(doc_content)}")
print(f"Total lines: {doc_content.count(chr(10))}")
