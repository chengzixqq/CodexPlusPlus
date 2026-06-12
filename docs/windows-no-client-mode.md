# Windows No-Client-Patch Mode

This fork mode keeps the official Codex Desktop package untouched. Codex++ owns
only user-level configuration, local marketplace mirrors, helper services, and
runtime CDP injection.

## Goals

- Do not patch, repack, resign, or write into the Codex Desktop MSIX package.
- Keep Microsoft Store / official Codex updates usable without reapplying an
  app bundle patch after each update.
- Keep the Codex plugin entry and plugin marketplace usable through Codex++
  runtime injection.
- Keep official bundled plugins, primary-runtime plugins, and a local curated
  marketplace registered through `%USERPROFILE%\.codex\config.toml`.
- Keep Computer Use compatibility as a user-level plugin/config/env repair, not
  an app package mutation.

## Non-Goals

- This mode does not bypass server-side account, region, OAuth, entitlement, or
  organization restrictions for official plugins.
- This mode does not fabricate new official plugins. The local marketplace can
  only expose plugins present in a local or downloaded marketplace manifest.
- This mode does not guarantee Fast Mode unless Codex itself or an upstream
  proxy sends `service_tier=priority`. Request-path mutation remains outside
  the no-client-patch boundary.

## Current Local Baseline

On the reference Windows machine:

- `openai-bundled` is a local marketplace with bundled plugins such as
  `browser`, `chrome`, `computer-use`, `latex`, and `sites`.
- `openai-primary-runtime` is a local runtime marketplace with artifact plugins
  such as `documents`, `presentations`, and `spreadsheets`.
- `openai-curated-local` is a local curated marketplace snapshot.
- `openai-curated` is left as the official remote marketplace so newly released
  official plugins can appear without refreshing the local snapshot.
- `openai-role-specific` is an optional local marketplace for OpenAI's public
  role-specific plugin templates from `openai/role-specific-plugins`.
- Codex++ injects the plugin entry, marketplace unlocks, and backend bridge via
  CDP. The page uses `window.__codexSessionDeleteBridge` for backend calls.

## Implemented in This Fork

- `codex-plus-core::marketplace_config` registers local marketplace sources in
  `%USERPROFILE%\.codex\config.toml` without mutating the Codex Desktop package.
- `codex-plus-core::computer_use_config` installs the local
  `computer-use@openai-bundled` compatibility plugin into the user-level
  `openai-bundled` mirror and stable cache.
- The bridge exposes `/plugin-marketplaces/repair`, `/computer-use/status`, and
  `/computer-use/repair`.
- The manager exposes "修复本地插件市场" in the enhancement page and Computer Use
  status/repair controls in the maintenance page.
- The launcher accepts `--app-path=...`, `--debug-port=...`, and
  `--helper-port=...`, matching the watcher argument style.

## Codex++ Responsibilities

Codex++ should own these Windows no-client-patch responsibilities:

1. **CDP launch and bridge stability**
   - Always launch or attach to Codex with a stable debug port.
   - If Codex is already running, verify the actual CDP port before deciding the
     launcher is already active.
   - Keep a watchdog that reinjects the renderer script and bridge when Codex
     reloads.

2. **Bridge routes**
   - Route `/backend/status`, `/settings/get`, `/settings/set`,
     `/thread-sort-keys`, `/thread-sort-key`, `/codex-model-catalog`,
     `/user-scripts/list`, `/user-scripts/set-enabled`,
     `/user-scripts/set-script-enabled`, `/user-scripts/reload`, and `/ads`
     through the Rust bridge, not through direct page `fetch` to localhost.
   - Return safe no-op success values for optional data routes when local data is
     unavailable, instead of surfacing 404s into the renderer.

3. **Marketplace registration**
   - Back up `%USERPROFILE%\.codex\config.toml` before edits.
   - Ensure these local marketplace entries exist when their source directories
     exist:
     - `openai-bundled`
     - `openai-primary-runtime`
     - `openai-curated-local`
     - `openai-role-specific`
   - Remove the old local `openai-curated` alias when it points at
     `openai-curated-local`, restoring the official remote marketplace.
   - Validate local marketplace manifests at both the marketplace root and
     `.agents\plugins\marketplace.json` where the Codex plugin loader expects
     them.

4. **Computer Use compatibility**
   - Ensure the local `computer-use@openai-bundled` plugin mirror exists.
   - Ensure the `computer-use` cache has a stable `latest` path.
   - Keep `browser` and `chrome` bundled plugin cache roots stable where those
     bundled plugins are present.
   - Ensure `[plugins."computer-use@openai-bundled"] enabled = true`.
   - Ensure `[features] computer_use = true` and
     `[features] remote_connections = true`.
   - Ensure `CODEX_ELECTRON_ENABLE_WINDOWS_COMPUTER_USE=1` is set at the user
     level.
   - Keep `[windows] sandbox = "unelevated"` in Codex config when the Windows
     sandbox refresh path fails with elevation errors.
   - Keep Chrome native messaging state pointed at a user-level stable chrome
     plugin cache when the Chrome native manifest already exists.

## What Stays Outside

- MSIX repacking and signing stays outside this mode.
- Fast Mode request-path patching stays outside this mode unless implemented as
  a Codex++ proxy/relay profile that deliberately injects `service_tier`.
- Official marketplace live refresh depends on the real `openai-curated`
  marketplace remaining remote. `openai-curated-local` is only a local fallback
  snapshot and needs a separate updater if it must be kept current offline.
- Public role-specific templates can be exposed through `openai-role-specific`,
  but templates that are not present in the public repository or the user's
  official remote marketplace are still outside the local repair boundary.
- Client-bundled feature gates and UI gates still stay outside pure user-level
  repair. This includes official app Statsig/feature checks for Computer Use UI,
  remote-control auth fallback, browser-use feature dispatch, and any hardcoded
  renderer/main-process gate that runs before Codex++ injection attaches.

## Implementation Plan

1. Done: add Windows local marketplace and Computer Use repair modules to
   `codex-plus-core`.
2. Done: expose them from the launcher bridge and manager UI.
3. Done: make watcher installation use stable argument formatting.
4. Partially done: move Computer Use compatibility behavior into Rust; remaining
   client feature gates require either a future proxy/bridge strategy or the
   existing client patch path.
5. Done: add tests for marketplace config preservation, Computer Use repair,
   route fallback payloads, and watcher argument parsing.
6. Done: stop redirecting `openai-curated` to the stale local snapshot; repair
   migrates old aliases back to the official remote marketplace.
7. Done: register an optional `openai-role-specific` local marketplace when the
   public OpenAI role-specific plugin template repository has been installed
   under `%USERPROFILE%\.codex\marketplaces\openai-role-specific`.
