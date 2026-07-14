//! Gate de aprovacao "preview before paste": depois de refinar, espera que o utilizador
//! aprove (Enter) ou recuse (Esc) antes de colar. A captura das teclas usa um low-level
//! keyboard hook do Windows (WH_KEYBOARD_LL) que CONSOME so o Enter/Esc durante o gate: assim
//! essas teclas nao vazam para a app em foco (o Enter nao mete newline no editor) e a overlay
//! nao precisa de roubar foco (a invariante sagrada: o paste aterra na app do utilizador).
//!
//! O `unsafe` do Win32 vive todo aqui, isolado. As pecas puras (`classify_key`, `Decision`,
//! `PREVIEW_TIMEOUT`) sao cross-platform e testadas em qualquer SO.

/// O que o utilizador decidiu no gate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    /// Aplicar: colar o texto refinado (Enter).
    Accept,
    /// Recusar: manter o original, nao colar nada (Esc, timeout, ou hotkey durante o gate).
    Reject,
}

/// Classificador puro: o que uma virtual-key premida significa no gate. Testavel em qualquer SO.
/// So o Enter e o Esc tem significado; tudo o resto passa (None = deixar seguir para a app).
pub fn classify_key(vk: u32) -> Option<Decision> {
    match vk {
        0x0D => Some(Decision::Accept), // VK_RETURN
        0x1B => Some(Decision::Reject), // VK_ESCAPE
        _ => None,
    }
}

/// Prazo total do gate: se o utilizador nao responder, recusa (mantem o original). O silencio
/// nunca vira um paste.
pub const PREVIEW_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// ---------------------------------------------------------------------------------------
// Windows: o hook real
// ---------------------------------------------------------------------------------------

#[cfg(windows)]
mod imp {
    use super::{classify_key, Decision, PREVIEW_TIMEOUT};
    use std::sync::atomic::{AtomicU8, Ordering};
    use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, MsgWaitForMultipleObjectsEx, PeekMessageW,
        SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK,
        KBDLLHOOKSTRUCT, MSG, MWMO_INPUTAVAILABLE, PM_REMOVE, QS_ALLINPUT, WH_KEYBOARD_LL,
        WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    // O callback e um `extern "system" fn` e nao captura estado: comunica pela decisao global.
    // 0 = pendente, 1 = accept, 2 = reject.
    static HOOK_DECISION: AtomicU8 = AtomicU8::new(0);
    // Ignorar teclas ja fisicamente premidas quando o hook instala (ex.: o Enter que disparou o
    // proprio refine). So contam apos uma transicao up->down fresca. Bit por tecla: 1=Enter,2=Esc.
    static IGNORE_HELD: AtomicU8 = AtomicU8::new(0);
    // Teclas cujo key-UP o hook ja viu nesta sessao de gate. Bits iguais aos de IGNORE_HELD.
    // O gate decide no key-DOWN, mas so LARGA o hook depois de ver a tecla subir: ver
    // `drain_until_released`.
    static RELEASED: AtomicU8 = AtomicU8::new(0);

    const IGN_ENTER: u8 = 1;
    const IGN_ESC: u8 = 2;

    fn ignore_bit(vk: u32) -> u8 {
        match vk {
            0x0D => IGN_ENTER,
            0x1B => IGN_ESC,
            _ => 0,
        }
    }

    unsafe extern "system" fn ll_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let msg = wparam.0 as u32;
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode;
            let bit = ignore_bit(vk);
            if bit != 0 {
                let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
                // Uma tecla que ja estava premida na instalacao: espera pelo key-up para limpar o
                // "ignorar", so a proxima descida conta. Enquanto isso, consome na mesma (nao deve
                // vazar para a app), mas nao decide.
                let ignoring = IGNORE_HELD.load(Ordering::SeqCst) & bit != 0;
                if is_up {
                    IGNORE_HELD.fetch_and(!bit, Ordering::SeqCst);
                    RELEASED.fetch_or(bit, Ordering::SeqCst);
                    return LRESULT(1); // consome o key-up de Enter/Esc para nao deixar cauda
                }
                if is_down {
                    if !ignoring {
                        if let Some(d) = classify_key(vk) {
                            HOOK_DECISION.store(
                                if d == Decision::Accept { 1 } else { 2 },
                                Ordering::SeqCst,
                            );
                        }
                    }
                    return LRESULT(1); // consome: a app em foco nunca ve este Enter/Esc
                }
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    /// Teto da espera pelo key-up da tecla da decisao. Uma tecla presa (ou um key-up que o hook
    /// nunca chega a ver) nunca pode pendurar o refine: ao fim disto seguimos na mesma.
    const RELEASE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

    /// Bombeia mensagens, com o hook AINDA INSTALADO, ate a tecla `bit` subir de verdade.
    ///
    /// Sem isto o gate devolvia `Accept` no key-DOWN do Enter e o hook caia logo a seguir: o
    /// resto daquela pressao (o key-up e, se o dedo demorasse uns milissegundos, as REPETICOES
    /// automaticas do Windows) chegava a app em foco sem ninguem a consumir. Num terminal isso
    /// era um Enter novo: o Claude Code submetia o prompt sozinho, antes de o utilizador sequer
    /// ver o texto colado. Enquanto o hook vive, o `ll_proc` engole tudo isso.
    ///
    /// Nao usa `GetAsyncKeyState`: o stream de eventos do proprio hook e a fonte da verdade (o
    /// GetAsyncKeyState ja provou mentir nesta app quando ha um hotkey global registado).
    fn drain_until_released(bit: u8) {
        let start = std::time::Instant::now();
        while RELEASED.load(Ordering::SeqCst) & bit == 0 {
            let mut msg = MSG::default();
            while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            if start.elapsed() >= RELEASE_TIMEOUT {
                log::warn!("gate: key-up never seen (bit={bit}); proceeding after timeout");
                return;
            }
            unsafe {
                MsgWaitForMultipleObjectsEx(None, 10, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
        }
        log::info!("gate: key released after {:?}", start.elapsed());
    }

    /// RAII: garante `UnhookWindowsHookEx` em todos os caminhos de saida (decisao, cancel,
    /// timeout, panic).
    struct HookGuard(HHOOK);
    impl Drop for HookGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = UnhookWindowsHookEx(self.0);
            }
        }
    }

    /// Corre o gate numa thread dedicada com message pump (o LL hook so entrega o callback na
    /// thread que instala E bombeia mensagens). Bloqueante: chamar fora do runtime tokio.
    pub fn run_gate_blocking(should_cancel: impl Fn() -> bool) -> Decision {
        HOOK_DECISION.store(0, Ordering::SeqCst);
        RELEASED.store(0, Ordering::SeqCst);
        // Marca as teclas ja premidas agora (bit alto do GetAsyncKeyState) para as ignorar ate
        // uma descida fresca. Evita um falso Accept do Enter que ainda estava em baixo.
        let mut held = 0u8;
        unsafe {
            if (GetAsyncKeyState(0x0D) as u16 & 0x8000) != 0 {
                held |= IGN_ENTER;
            }
            if (GetAsyncKeyState(0x1B) as u16 & 0x8000) != 0 {
                held |= IGN_ESC;
            }
        }
        IGNORE_HELD.store(held, Ordering::SeqCst);

        log::info!("gate: starting (held_at_install={held})");
        let hmod = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
        let hook = match unsafe {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_proc), Some(HINSTANCE(hmod.0)), 0)
        } {
            Ok(h) => h,
            // Nao conseguimos instalar o hook: degrada para colar (nunca perde um refine bom).
            Err(e) => {
                log::warn!("gate: HOOK INSTALL FAILED ({e}); pasting without approval");
                return Decision::Accept;
            }
        };
        let _guard = HookGuard(hook);
        let start = std::time::Instant::now();

        loop {
            // 1) Bombeia mensagens: serve o callback do LL hook.
            let mut msg = MSG::default();
            while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            // 2) Decisao vinda do callback? Antes de largar o hook, espera o key-up REAL da
            //    tecla premida: enquanto o dedo estiver em baixo, o hook tem de continuar a
            //    engolir as repeticoes automaticas, senao vazam para a app (num terminal, um
            //    Enter vazado submete o prompt sozinho).
            match HOOK_DECISION.load(Ordering::SeqCst) {
                1 => {
                    log::info!("gate: ACCEPT (Enter consumed by hook)");
                    drain_until_released(IGN_ENTER);
                    return Decision::Accept;
                }
                2 => {
                    log::info!("gate: REJECT (Esc consumed by hook)");
                    drain_until_released(IGN_ESC);
                    return Decision::Reject;
                }
                _ => {}
            }
            // 3) Cancel externo (hotkey durante o preview) -> recusa.
            if should_cancel() {
                log::info!("gate: REJECT (cancelled)");
                return Decision::Reject;
            }
            // 4) Prazo total -> recusa (nunca colar sem aprovacao explicita).
            if start.elapsed() >= PREVIEW_TIMEOUT {
                log::info!("gate: REJECT (timeout, no key seen)");
                return Decision::Reject;
            }
            // 5) Espera eficiente: acorda ja no input (Enter/Esc imediato), senao 50ms para
            //    re-checar cancel/prazo. Mantem o LL hook responsivo (callback trivial, nunca
            //    estoura o LowLevelHooksTimeout ~300ms).
            unsafe {
                MsgWaitForMultipleObjectsEx(None, 50, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
        }
        // _guard cai aqui -> UnhookWindowsHookEx
    }

    /// Wrapper async: spawna a thread do gate e espera o resultado por oneshot (race-free).
    pub async fn gate(app: tauri::AppHandle) -> Decision {
        use tauri::Manager;
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let d = run_gate_blocking(|| {
                app.state::<crate::state::AppState>()
                    .cancel
                    .load(Ordering::SeqCst)
            });
            let _ = tx.send(d); // se o lado async caiu (app a sair), o send falha inofensivamente
        });
        rx.await.unwrap_or(Decision::Reject)
    }
}

#[cfg(windows)]
pub use imp::gate;

/// Non-Windows: nao ha hook. Ember e Windows-first; aqui degrada para o comportamento antigo
/// (cola direto), sem hook, sem descarte silencioso, sem meio-event-tap de macOS.
#[cfg(not(windows))]
pub async fn gate(_app: tauri::AppHandle) -> Decision {
    Decision::Accept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_maps_enter_and_esc_only() {
        assert_eq!(classify_key(0x0D), Some(Decision::Accept));
        assert_eq!(classify_key(0x1B), Some(Decision::Reject));
        assert_eq!(classify_key(0x41), None); // 'A' passa
        assert_eq!(classify_key(0x20), None); // Space passa
    }
}
