# Egui Component Inventory for Shadows of War

This document provides a starter kit of `egui` components extracted and audited from the `egui_demo_app` and core `egui` library, categorized by their utility in game screens.

## 1. Core Layout Containers
Essential for structuring the different screens of the game.

| Component | Description | Best Use Case |
| :--- | :--- | :--- |
| `CentralPanel` | The main content area that fills the remaining screen space. | Main background for any screen. |
| `TopBottomPanel` | A panel fixed to the top or bottom. | Resource bars (HUD), Bottom status bar, Top menu bar. |
| `SidePanel` | A panel fixed to the left or right. | Command panels (HUD), Chat windows, Lobby player list. |
| `Window` | A floating, draggable, and resizable container. | Modals (Victory/Defeat), Popups, In-game settings. |
| `Area` | A freely positioned container without a frame. | Minimap overlay, floating damage numbers, contextual icons. |
| `ScrollArea` | Adds scrollbars to its content. | Lobby browser, Chat history, long Settings pages. |

## 2. Basic Widgets
The building blocks of interactive elements.

| Component | Description | Best Use Case |
| :--- | :--- | :--- |
| `Button` | Standard clickable button. | "Start Game", "Join Lobby", "Attack" command. |
| `Label` | Display text. | Title text, resource counts, unit names. |
| `RichText` | Text with custom color, size, and style. | "VICTORY" (Large/Gold), "DEFEAT" (Large/Red). |
| `Hyperlink` | Clickable link to external URL. | "Discord", "Wiki", "Report Bug". |
| `Image` | Displays a texture. | Unit icons, map previews, logo. |
| `Spinner` | Loading animation. | "Connecting to server...", "Loading map...". |
| `Separator` | Horizontal or vertical line. | Grouping lobby info, separating HUD sections. |

## 3. Input & Controls
For gathering player input and adjusting settings.

| Component | Description | Best Use Case |
| :--- | :--- | :--- |
| `TextEdit` | Single or multi-line text input. | Player nickname, Chat message, Server IP input. |
| `Checkbox` | Toggle state. | "Ready" status in lobby, "Enable Music" in settings. |
| `RadioButton` | Select one from many. | Team selection (Red vs Blue), Graphics quality. |
| `ComboBox` | Dropdown selection. | Map selection, Resolution selection. |
| `Slider` | Numeric range selection. | Volume control, Camera sensitivity. |
| `DragValue` | Numeric input by dragging or typing. | Precise troop count selection, Custom game settings. |

## 4. Advanced & Specialized
Complex components for data-heavy views.

| Component | Description | Best Use Case |
| :--- | :--- | :--- |
| `Grid` | Simple 2D layout. | Stats summary screen, Player list with columns. |
| `Table` | High-performance scrolling table. | Lobby browser with many games and sortable columns. |
| `CollapsingHeader` | Expandable/collapsible section. | Nested settings, Unit ability details. |
| `ProgressBar` | Visual progress indicator. | Unit training progress, Map download % |
| `ColorPicker` | UI for selecting colors. | Customizing player/team color. |

---

## 5. Screen-Specific Starter Kit

### Main Menu & Lobbies
- **Lobby Browser**: Use `ScrollArea` + `Grid` (or `Table`) to list active games. Use `TextEdit` for a search filter at the top.
- **Lobby Room**: `SidePanel` for the player list. `CentralPanel` for map preview and settings. A large "READY" `Button` with `Color32` background.
- **Queue Overlay**: A `Window` with `Spinner` and a "Cancel" `Button`.

### Gameplay HUD
- **Top Bar**: `TopBottomPanel` with `horizontal` layout for Gold, Troops, and Time. Use `RichText` for emphasis.
- **Command Card**: `SidePanel` (right) with a `Grid` of `ImageButton` for unit commands.
- **Minimap**: `Area` (bottom left) with a custom `Painter` call for drawing the map.

### Victory / Defeat Screen
- **Modal Window**: A non-resizable `Window` in the center.
- **Stats Table**: A `Grid` showing "Territory Captured", "Troops Lost", "Gold Earned".
- **Actions**: "Play Again" (Primary button), "Back to Menu" (Secondary).

### Disconnect / Error
- **Simple Window**: Modal window with `RichText::color(Color32::RED)` for the error message.
- **Auto-Reconnect**: A `Spinner` next to a "Retrying..." label.

## 6. Prototyping Patterns (from egui_demo_app)
- **Global Theme Preference**: `egui::widgets::global_theme_preference_switch(ui)` for Light/Dark mode.
- **FPS Overlay**: Copy `FrameHistory` logic from `backend_panel.rs` to show performance.
- **Debug Inspection**: Use `ctx.inspection_ui(ui)` during development to debug UI layouts.
- **Settings UI**: Use `ctx.settings_ui(ui)` as a base for the game's own settings menu.
