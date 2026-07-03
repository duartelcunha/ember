# Ember

In-the-moment prompt refiner for any app. Select text anywhere, press the global
shortcut, and Ember refines the selection in place: a small orb loads next to your
cursor, the text is rewritten by an LLM, and your selection is replaced. No window,
no copy-paste dance.

- **Auto refine in place.** Hotkey on a selection captures it (via clipboard), refines
  it, and pastes the result back over the selection. Your original clipboard is restored.
- **Resilient by design.** Gemini is primary, Claude is the fallback (different families
  fail for different reasons), with transient retry and provider fallback on exhaustion.
- **BYOK.** Your API keys live in the Windows Credential Manager, never in plain text.
- **Guided by your profile.** Auto-detected from your `CLAUDE.md`, or edited in Settings.

## Stack

Tauri 2 (Rust shell) + React 19 / Vite / Tailwind 4. The pure logic (refine pipeline,
selection sequencing) lives in the `ember-core` crate and is unit-tested without I/O.

## Develop

```bash
npm install
npm run tauri dev
```

The app runs in the system tray. Default shortcut: `Ctrl+Shift+Space`. Open Settings from
the tray to add a key and tune the profile.

## Test

```bash
cargo test -p ember-core
```

## Versioning & releases

`package.json` is the single source of truth for the version: `src-tauri/tauri.conf.json`
reads it directly, and the Cargo workspace mirrors it (`Cargo.toml`'s
`[workspace.package] version`, with every crate inheriting via `version.workspace = true`).
Run `npm run version:check` to verify the two are in sync (or `-- --write` to fix a manual
drift).

Commits follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`,
`fix:`, `chore:`, ...). [release-please](https://github.com/googleapis/release-please)
reads them to maintain a standing release PR that bumps the version, regenerates
`CHANGELOG.md`, and tags `vX.Y.Z` on merge.

## Auto-update

The app checks `https://github.com/<owner>/ember/releases/latest/download/latest.json`
(Settings -> About -> Check for updates) and verifies the update signature against the
public key baked into `tauri.conf.json`. A single workflow, `.github/workflows/release.yml`
(on push to `main`), produces a release in two jobs:

1. `release-please` opens/updates the standing release PR; merging it tags `vX.Y.Z` and
   publishes the GitHub Release as a **prerelease** with the changelog.
2. `build-and-upload` (same workflow, gated on `release_created`) runs the tests, builds the
   signed NSIS installer and updater artifacts, uploads them, and only then promotes the
   release to the latest full release (`gh release edit --prerelease=false --latest`).

The prerelease-until-artifacts step closes the window where the release exists but its
`latest.json` does not: `releases/latest` skips prereleases, so during the build users keep
seeing the previous full release instead of a 404. If the build fails, the release stays a
prerelease and is never served to anyone.

The two jobs live in one workflow on purpose: a tag created with `GITHUB_TOKEN` does not
cascade to trigger a separate `push: tags:` workflow, so the build has to run in the same
run that created the tag. `.github/workflows/ci.yml` runs the same tests on every PR.

The build needs two repo secrets: `TAURI_SIGNING_PRIVATE_KEY` (contents of the
`.key` file from `tauri signer generate`) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
Losing the private key means existing installs can no longer verify future updates.

### Rolling back a bad release

The updater only ever moves forward (it installs a remote version higher than the local one),
so there is no auto-downgrade. Two levers:

- **Stop it spreading.** Mark the bad release as a prerelease again
  (`gh release edit vX.Y.Z --prerelease=true`) or delete it. `releases/latest` immediately
  falls back to the previous full release, so anyone who has not updated yet stays on the
  good version.
- **Fix users already on it.** Roll forward: land the fix and cut a higher patch version. The
  updater offers it on the next check. There is no way to pull a bad build back off a machine
  that already installed it, so the prerelease gate above is the main line of defense.

### Code signing and SmartScreen

The NSIS installer is signed for the *updater* (minisign, the `TAURI_SIGNING_*` secrets), not
with an Authenticode certificate, so first-time installs show Windows SmartScreen's "unknown
publisher" warning until the download builds reputation. Options, cheapest first:

- **Azure Trusted Signing** (~a few USD/month) issues short-lived Authenticode certs and is the
  modern replacement for a one-off EV cert. Wire it into `tauri.conf.json`
  (`bundle.windows.signCommand`) so the CI build signs the installer.
- **winget / Microsoft Store** distribution sidesteps the raw-download SmartScreen prompt.
- **Do nothing** and let download volume build SmartScreen reputation over time (slow, and it
  resets whenever the signing identity changes).

The minisign updater signature and an Authenticode signature are independent: the first proves
an update came from you, the second is what SmartScreen and the OS trust at install time.
