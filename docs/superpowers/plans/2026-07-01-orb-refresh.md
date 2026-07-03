# Orb Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the orb getting stuck at the screen edge when the cursor crosses to another
monitor, shrink it, and swap its rotating "loading spinner" look for a small, smooth,
Claude-tinted pulsing glow.

**Architecture:** One pure-logic fix in `ember-core` (find the monitor that contains a
point, instead of trusting the window's stale "current monitor"), one call-site change in
`src-tauri/src/lib.rs` to use it, and a CSS/component-only visual change in the overlay
frontend (`Orb.tsx` + `globals.css`). No IPC/capability changes needed: the monitor APIs
used here (`cursor_position`, `available_monitors`) are called directly from Rust host
code, the same way `current_monitor` already is today.

**Tech Stack:** Rust (Tauri 2, workspace crates `ember-core` + `ember` in `src-tauri`),
React + TypeScript + Motion (`motion/react`) + Tailwind v4 CSS variables.

---

## File Structure

- `crates/ember-core/src/selection.rs`: add `monitor_containing`, a pure function (point +
  list of monitor rects → the rect containing the point). Lives next to the existing
  `clamp_pos`, tested the same way (no OS, no Tauri types).
- `src-tauri/src/lib.rs`: add `monitor_at_point` (Tauri-facing: reads
  `w.available_monitors()`, converts to plain tuples, delegates to
  `ember_core::selection::monitor_containing`, falls back to the existing
  `monitor_work_area` if the point isn't inside any monitor). `orb_target` calls this
  instead of `monitor_work_area`. `ORB_OFFSET` shrinks to match the smaller orb.
- `src/styles/globals.css`: new `--color-orb-accent` theme var (Claude terracotta, used
  only by the orb, `--color-accent` untouched); `.ember-orb` shrinks and recolors;
  `.ember-ring` (rotating conic-gradient arc) is replaced by `.ember-glow` (radial blur,
  no rotation).
- `src/overlay/Orb.tsx`: smaller outer box; renders `.ember-glow` (opacity+scale pulse)
  instead of the rotating `.ember-ring`.

---

## Task 1: `monitor_containing` in ember-core (TDD)

**Files:**
- Modify: `crates/ember-core/src/selection.rs:77` (insert function before `#[cfg(test)]`)
- Modify: `crates/ember-core/src/selection.rs:166` (insert tests before the closing `}` of `mod tests`, right after `clamp_keeps_window_on_screen`)
- Test: same file (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing tests**

Open `crates/ember-core/src/selection.rs`. In `mod tests`, right after the
`clamp_keeps_window_on_screen` test (ends at line 166, just before the module's closing
`}`), add:

```rust
    #[test]
    fn monitor_containing_finds_point_in_first_monitor() {
        let monitors = [(0, 0, 1920, 1080), (1920, 0, 1920, 1080)];
        assert_eq!(
            monitor_containing(100, 100, &monitors),
            Some((0, 0, 1920, 1080))
        );
    }

    #[test]
    fn monitor_containing_finds_point_in_second_monitor() {
        let monitors = [(0, 0, 1920, 1080), (1920, 0, 1920, 1080)];
        assert_eq!(
            monitor_containing(2500, 500, &monitors),
            Some((1920, 0, 1920, 1080))
        );
    }

    #[test]
    fn monitor_containing_treats_left_edge_as_inside_and_right_edge_as_outside() {
        let monitors = [(0, 0, 1920, 1080), (1920, 0, 1920, 1080)];
        // x=1920 e o primeiro pixel do segundo monitor, nao o ultimo do primeiro.
        assert_eq!(
            monitor_containing(1920, 0, &monitors),
            Some((1920, 0, 1920, 1080))
        );
        // x=1919 e o ultimo pixel do primeiro monitor.
        assert_eq!(
            monitor_containing(1919, 0, &monitors),
            Some((0, 0, 1920, 1080))
        );
    }

    #[test]
    fn monitor_containing_returns_none_outside_all_monitors() {
        let monitors = [(0, 0, 1920, 1080)];
        assert_eq!(monitor_containing(-10, 500, &monitors), None);
        assert_eq!(monitor_containing(500, 2000, &monitors), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p ember-core monitor_containing`
Expected: FAIL to compile (`cannot find function `monitor_containing` in this scope`).

- [ ] **Step 3: Implement `monitor_containing`**

In the same file, right before `#[cfg(test)]` (currently line 79), add:

```rust
/// Encontra o monitor (retangulo x,y,w,h) que contem o ponto (px,py), tipicamente o
/// cursor. Usado para clampar o orb ao ecra ONDE O CURSOR ESTA, nao ao ecra da janela
/// (que fica desatualizado quando o cursor muda de monitor a meio do seguimento).
pub fn monitor_containing(
    px: i32,
    py: i32,
    monitors: &[(i32, i32, i32, i32)],
) -> Option<(i32, i32, i32, i32)> {
    monitors
        .iter()
        .copied()
        .find(|&(x, y, w, h)| px >= x && px < x + w && py >= y && py < y + h)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p ember-core monitor_containing`
Expected: PASS, 4 tests ok (plus the rest of the `ember-core` suite unaffected).

- [ ] **Step 5: Commit**

```bash
git add crates/ember-core/src/selection.rs
git commit -m "feat(core): add monitor_containing for cursor-based monitor lookup"
```

---

## Task 2: Fix the multi-monitor follow bug in `lib.rs`

**Files:**
- Modify: `src-tauri/src/lib.rs:23` (`ORB_OFFSET`)
- Modify: `src-tauri/src/lib.rs:44-53` (insert `monitor_at_point` after `monitor_work_area`)
- Modify: `src-tauri/src/lib.rs:65` (`orb_target` uses the new function)

- [ ] **Step 1: Shrink `ORB_OFFSET` to match the smaller orb (Task 3 shrinks it to 18px)**

Change:

```rust
/// Offset do orb em relacao ao cursor (centro do orb ~ cursor + isto), em px fisicos.
const ORB_OFFSET: i32 = 26;
```

to:

```rust
/// Offset do orb em relacao ao cursor (centro do orb ~ cursor + isto), em px fisicos.
const ORB_OFFSET: i32 = 18;
```

- [ ] **Step 2: Add `monitor_at_point`, right after `monitor_work_area` (before `orb_target`)**

```rust
/// Geometria do monitor que contem o ponto (px,py), tipicamente o cursor. Ao contrario
/// de `monitor_work_area`, nao depende de onde a janela esta agora, por isso o orb
/// consegue atravessar para outro ecra em vez de ficar preso na borda do monitor de
/// origem quando o cursor muda de ecra a meio do seguimento.
fn monitor_at_point(w: &WebviewWindow, px: i32, py: i32) -> (i32, i32, i32, i32) {
    let monitors: Vec<(i32, i32, i32, i32)> = w
        .available_monitors()
        .map(|ms| {
            ms.iter()
                .map(|m| {
                    let p = m.position();
                    let s = m.size();
                    (p.x, p.y, s.width as i32, s.height as i32)
                })
                .collect()
        })
        .unwrap_or_default();
    ember_core::selection::monitor_containing(px, py, &monitors)
        .unwrap_or_else(|| monitor_work_area(w))
}
```

- [ ] **Step 3: Use it in `orb_target`**

In `orb_target`, change:

```rust
    let (ax, ay, aw, ah) = monitor_work_area(w);
```

to:

```rust
    let (ax, ay, aw, ah) = monitor_at_point(w, c.x as i32, c.y as i32);
```

(`c` is already bound earlier in the function as `app.cursor_position().ok()?`.)

- [ ] **Step 4: Compile check**

Run: `cargo check -p ember`
Expected: compiles clean, no warnings about unused `monitor_work_area` (it's still used as
the fallback inside `monitor_at_point`).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: clamp orb to the monitor under the cursor, not the window's stale monitor"
```

---

## Task 3: Shrink the orb, retint it, replace the spinner with a glow

**Files:**
- Modify: `src/styles/globals.css:17` (new theme var), `:74-81` (`.ember-orb`), `:83-97` (`.ember-ring` → `.ember-glow`)
- Modify: `src/overlay/Orb.tsx` (full file)

- [ ] **Step 1: Add the orb-only accent var**

In `src/styles/globals.css`, inside the `@theme { ... }` block, right after
`--color-accent-fg: #1a0e03;` (line 17), add:

```css
  /* Accent so do orb (terracota Claude). --color-accent (Settings/Logo) fica inalterado. */
  --color-orb-accent: #d97757;
```

- [ ] **Step 2: Shrink and recolor `.ember-orb`**

Replace:

```css
.ember-orb {
  width: 26px;
  height: 26px;
  border-radius: 9999px;
  background: var(--color-accent);
  box-shadow: 0 1px 10px rgba(255, 122, 24, 0.45);
  will-change: transform;
}
```

with:

```css
.ember-orb {
  width: 18px;
  height: 18px;
  border-radius: 9999px;
  background: var(--color-orb-accent);
  box-shadow: 0 1px 8px rgba(217, 119, 87, 0.45);
  will-change: transform;
}
```

- [ ] **Step 3: Replace the rotating ring with a pulsing glow**

Replace:

```css
/* Anel spinner flat a rodar (estado a refinar), por cima do orb. So roda (transform). */
.ember-ring {
  position: absolute;
  inset: -7px;
  border-radius: 9999px;
  background: conic-gradient(
    from 0deg,
    transparent 0deg,
    var(--color-accent) 70deg,
    transparent 200deg
  );
  -webkit-mask: radial-gradient(farthest-side, transparent calc(100% - 2.5px), #000 calc(100% - 2.5px));
  mask: radial-gradient(farthest-side, transparent calc(100% - 2.5px), #000 calc(100% - 2.5px));
  will-change: transform;
}
```

with:

```css
/* Glow pulsante (estado "a pensar"), por baixo do orb. Sem rotacao: so respiracao de luz. */
.ember-glow {
  position: absolute;
  inset: -7px;
  border-radius: 9999px;
  background: radial-gradient(circle, rgba(217, 119, 87, 0.55) 0%, rgba(217, 119, 87, 0) 70%);
  filter: blur(2px);
  will-change: opacity, transform;
}
```

- [ ] **Step 4: Rewrite `Orb.tsx`**

Replace the full contents of `src/overlay/Orb.tsx` with:

```tsx
import { m } from "motion/react";

/**
 * O orb (estado "a pensar"): bolinha terracota 2D + glow pulsante por baixo.
 * Partilha `layoutId` com a pilha para o morph. Pulse = scale (compositor).
 */
export function Orb() {
  return (
    <m.div
      layoutId="refiner-surface"
      className="relative grid place-items-center"
      style={{ borderRadius: 9999, width: 28, height: 28 }}
      initial={{ opacity: 0, scale: 0.6 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.6 }}
    >
      <m.div
        className="ember-glow"
        animate={{ opacity: [0.5, 1, 0.5], scale: [0.9, 1.15, 0.9] }}
        transition={{ repeat: Infinity, duration: 1.6, ease: "easeInOut" }}
      />
      <m.div
        className="ember-orb"
        animate={{ scale: [1, 1.1, 1] }}
        transition={{ repeat: Infinity, duration: 1.6, ease: "easeInOut" }}
      />
    </m.div>
  );
}
```

- [ ] **Step 5: Typecheck + build**

Run: `npm run build`
Expected: `tsc` and `vite build` both succeed, no errors (no other file references
`.ember-ring` or the old orb size, confirmed by grep during design).

- [ ] **Step 6: Commit**

```bash
git add src/styles/globals.css src/overlay/Orb.tsx
git commit -m "feat(overlay): shrink orb, retint to Claude terracotta, swap spinner for a pulsing glow"
```

---

## Task 4: End-to-end manual verification (run skill)

**Files:** none (verification only)

- [ ] **Step 1: Launch**

Run: `npm run tauri dev` (kill any stale process holding port 1420 first if the command
errors with "Port 1420 is already in use").
Expected: builds clean, tray icon appears, `Running ember.exe` with no panics.

- [ ] **Step 2: Visual check**

Select some text in any app and trigger the refine hotkey. While the orb is in its
"refining" (thinking) state:
Expected: the orb is visibly smaller (~18px) than before, terracotta-colored (not the
ember orange used elsewhere in the app), and glows with a smooth pulsing halo (no
rotating arc/spinner piece).

- [ ] **Step 3: Multi-monitor check**

If two monitors are available: trigger the hotkey on one monitor, then, while the orb is
following (or by moving the mouse right after triggering), drag the cursor across to the
second monitor.
Expected: the orb crosses over and keeps following on the second monitor, instead of
stopping at the shared edge. If only one monitor is available, note this in the final
report as unverified and rely on Task 1's unit tests plus code review for confidence.

- [ ] **Step 4: Regression check**

Trigger the hotkey with nothing selected, and again with no API key configured.
Expected: the "Select text first" hint and the "No API key" error still show and behave
as before. This task didn't touch that logic, only the orb's geometry/visuals.

- [ ] **Step 5: Final report**

Summarize what was verified, any rough edges (e.g. glow intensity/timing feels off), and
whether a follow-up visual tweak pass is worth doing.

---

## Self-Review

**Spec coverage:** multi-monitor fix (Task 1 pure logic + Task 2 wiring) ✓; smaller ball
(Task 3, 26px→18px + proportional outer box + `ORB_OFFSET` in Task 2) ✓; Claude terracotta
color scoped to the orb only (Task 3, new `--color-orb-accent`, `--color-accent` untouched)
✓; glow instead of rotating spinner (Task 3, `.ember-ring`→`.ember-glow`, no `rotate`
animation) ✓; testable monitor lookup (Task 1, pure function + 4 unit tests, no OS/Tauri
types) ✓; manual verification incl. multi-monitor (Task 4) ✓; scope guard: no changes to
`Pill.tsx`, Settings, or `--color-accent` (confirmed via grep before writing the plan: only
`Orb.tsx` and `globals.css` reference `.ember-ring`/`.ember-orb`) ✓.

**Placeholder scan:** no TBD/TODO; every step shows complete, real code. The only
explicitly deferred detail (glow intensity/timing feels tuned by eye in Task 4 rather than
pre-committed to a "correct" number) was called out as intentional in the design doc, not
an oversight.

**Type consistency:** `monitor_containing(px, py, monitors: &[(i32,i32,i32,i32)]) -> Option<(i32,i32,i32,i32)>`
defined in Task 1 and called identically in Task 2's `monitor_at_point`. `monitor_at_point`
defined in Task 2 and used in `orb_target` with the same signature
`(w: &WebviewWindow, px: i32, py: i32) -> (i32, i32, i32, i32)`. CSS class rename
`.ember-ring` → `.ember-glow` is applied consistently in both `globals.css` (Task 3 Step 3)
and `Orb.tsx` (Task 3 Step 4): no leftover reference to the old class name. `--color-orb-accent`
introduced in Task 3 Step 1 and consumed in Step 2; the literal `rgba(217, 119, 87, ...)`
glow values match `#d97757` (217, 119, 87 in decimal), same convention the file already
uses for `--color-accent`'s box-shadow.
