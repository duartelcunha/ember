//! Control flow puro de retry + fallback. O coracao da resiliencia.
//!
//! Tres funcoes puras (`classify`, `backoff_ms`, `plan`) que carregam 100% da
//! ramificacao. O orquestrador (em `src-tauri`) e ~15 linhas e so faz I/O. Assim o
//! standard de resiliencia fica totalmente testavel sem rede (provider-fallback-on-
//! transient-errors / STACK-07).

use crate::error::{CoreError, OutcomeClass};
use serde::{Deserialize, Serialize};

/// Configuracao da maquina de resiliencia.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryConfig {
    /// Quantos providers existem na cadeia (ate 3: Gemini primario, OpenAI-compatible fallback,
    /// Claude opcional). No runtime e sempre redefinido para `chain.len()` em `commands::refine_text`.
    pub provider_count: usize,
    /// Retries por provider antes de passar ao seguinte (em erros transitorios).
    pub max_retries_per_provider: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    /// Fracao de jitter aplicada ao backoff (ex: 0.25 = ate +25%).
    pub jitter_frac: f64,
    /// Se `true`, uma recusa por politica tenta a outra familia de provider.
    pub fallback_on_content_policy: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            provider_count: 3,
            max_retries_per_provider: 2,
            base_delay_ms: 400,
            max_delay_ms: 8_000,
            jitter_frac: 0.25,
            fallback_on_content_policy: false,
        }
    }
}

/// Estado da maquina: em que provider e em que tentativa estamos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopState {
    pub provider_index: usize,
    pub attempt: u32,
}

impl LoopState {
    pub fn start() -> Self {
        Self {
            provider_index: 0,
            attempt: 0,
        }
    }
}

/// O que fazer a seguir, dada a ultima tentativa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Sucesso: devolve o resultado.
    Succeed,
    /// Repetir o mesmo provider apos `delay_ms`.
    Retry { delay_ms: u64, next: LoopState },
    /// Passar ao provider seguinte (reset da tentativa).
    Fallback { next: LoopState },
    /// Desistir, propagando a razao sem mascarar.
    Fail { reason: CoreError },
}

/// Classifica uma resposta HTTP numa `OutcomeClass` (com base no status code).
///
/// Content-policy (200 + recusa) e detetado pelo body, nao aqui: o orquestrador usa
/// `providers::*_is_content_policy` e constroi `OutcomeClass::ContentPolicy` diretamente.
/// `api_error_code` fica disponivel para refinamento futuro (mapeamos sobretudo o status).
pub fn classify(
    _provider: crate::model::Provider,
    http_status: u16,
    _api_error_code: Option<&str>,
    retry_after_ms: Option<u64>,
) -> OutcomeClass {
    match http_status {
        200 => OutcomeClass::Success,
        // Transitorios explicitos: timeout/conflito/rate-limit/erros de servidor/overload.
        408 | 409 | 429 | 500 | 502 | 503 | 504 | 529 => {
            OutcomeClass::Transient { retry_after_ms }
        }
        // Credencial: nao faz retry cego; dispara fallback (chave diferente no outro).
        401 | 403 => OutcomeClass::Auth,
        // O MODELO nao existe (descontinuado, ou id mal escrito). Nao e um bug do pedido: o
        // provider seguinte usa um modelo diferente e nao sabe nada deste, por isso vale a pena
        // tentar. Regressao real: a Google descontinuou o `gemini-2.5-flash-lite` e o 404, tratado
        // como Payload, matava a CADEIA INTEIRA sem sequer tocar no fallback.
        404 => OutcomeClass::ModelNotFound,
        // Bug nosso no pedido: propaga sem mascarar.
        400 | 413 | 422 => OutcomeClass::Payload,
        // Resto: 5xx desconhecido -> transitorio; 4xx desconhecido -> payload.
        s if (500..=599).contains(&s) => OutcomeClass::Transient { retry_after_ms },
        _ => OutcomeClass::Payload,
    }
}

/// Backoff exponencial com jitter. `rng01` e injetado em [0,1) para determinismo nos testes
/// (sem `rand`, sem `Instant` aqui dentro).
///
/// O `Retry-After` do servidor e um PISO, nunca algo a encurtar. Antes fazia-se
/// `server.min(max_delay)`: com o servidor a pedir 12s e o nosso teto em 8s, esperavamos 8s e
/// batiamos-lhe DENTRO do cooldown, garantindo outro 429 e queimando as tentativas todas por
/// nada (visto em producao contra os modelos `:free` do OpenRouter). Um `Retry-After: 0` (que o
/// Gemini devolve) tambem nao pode virar um retry instantaneo: pisa-se no `base_delay_ms`.
///
/// Quem decide NAO esperar um `Retry-After` grande demais e o `plan` (cai para o provider
/// seguinte); aqui, se chega, honra-se.
pub fn backoff_ms(
    attempt: u32,
    cfg: &RetryConfig,
    rng01: f64,
    server_retry_after_ms: Option<u64>,
) -> u64 {
    if let Some(server) = server_retry_after_ms {
        return server.max(cfg.base_delay_ms);
    }
    let factor = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
    let capped = cfg.base_delay_ms.saturating_mul(factor).min(cfg.max_delay_ms);
    let jitter = (capped as f64) * cfg.jitter_frac * rng01.clamp(0.0, 1.0);
    ((capped as f64) + jitter).min(cfg.max_delay_ms as f64) as u64
}

/// A funcao de transicao. Pura: dado o estado e o outcome, devolve a proxima `Decision`.
pub fn plan(state: &LoopState, outcome: &OutcomeClass, cfg: &RetryConfig, rng01: f64) -> Decision {
    let has_next_provider = state.provider_index + 1 < cfg.provider_count;
    let fallback = || Decision::Fallback {
        next: LoopState {
            provider_index: state.provider_index + 1,
            attempt: 0,
        },
    };

    match outcome {
        OutcomeClass::Success => Decision::Succeed,
        OutcomeClass::Payload => Decision::Fail {
            reason: CoreError::Payload,
        },
        OutcomeClass::ContentPolicy => {
            if cfg.fallback_on_content_policy && has_next_provider {
                fallback()
            } else {
                Decision::Fail {
                    reason: CoreError::ContentPolicy,
                }
            }
        }
        OutcomeClass::Auth => {
            if has_next_provider {
                fallback()
            } else {
                Decision::Fail {
                    reason: CoreError::Auth,
                }
            }
        }
        // Modelo inexistente: repetir da o mesmo 404, mas a familia seguinte tem outro modelo.
        OutcomeClass::ModelNotFound => {
            if has_next_provider {
                fallback()
            } else {
                Decision::Fail {
                    reason: CoreError::ModelNotFound,
                }
            }
        }
        // Corte por tokens e deterministico no mesmo provider (retry cego devolveria o mesmo
        // corte), mas o outro provider pode ter mais folga: fallback, sem retry.
        OutcomeClass::Truncated => {
            if has_next_provider {
                fallback()
            } else {
                Decision::Fail {
                    reason: CoreError::Truncated,
                }
            }
        }
        OutcomeClass::Transient { retry_after_ms } => {
            // O servidor pede mais tempo do que estamos dispostos a esperar? Nao vale a pena
            // insistir NESTE provider: o Ember refina no momento, ninguem espera 30s por um
            // paragrafo. Esperar menos do que o pedido so daria outro rate-limit. Vamos direto
            // a familia seguinte, que tem chave e limites proprios. Sem outra familia, falha
            // honestamente em vez de prender o utilizador num sleep longo.
            let asks_too_long = retry_after_ms.is_some_and(|ra| ra > cfg.max_delay_ms);
            if asks_too_long {
                return if has_next_provider {
                    fallback()
                } else {
                    Decision::Fail {
                        reason: CoreError::AllProvidersFailed,
                    }
                };
            }
            if state.attempt < cfg.max_retries_per_provider {
                Decision::Retry {
                    delay_ms: backoff_ms(state.attempt, cfg, rng01, *retry_after_ms),
                    next: LoopState {
                        provider_index: state.provider_index,
                        attempt: state.attempt + 1,
                    },
                }
            } else if has_next_provider {
                fallback()
            } else {
                Decision::Fail {
                    reason: CoreError::AllProvidersFailed,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Provider;

    fn cfg() -> RetryConfig {
        RetryConfig::default()
    }

    /// Config de 2 providers (Gemini + 1 fallback), para os testes que verificam o comportamento
    /// "ultimo provider" sem depender do default atual (agora 3).
    fn cfg2() -> RetryConfig {
        RetryConfig {
            provider_count: 2,
            ..RetryConfig::default()
        }
    }

    #[test]
    fn classify_maps_status_codes() {
        let g = Provider::Gemini;
        assert_eq!(classify(g, 200, None, None), OutcomeClass::Success);
        assert_eq!(
            classify(g, 429, None, Some(1500)),
            OutcomeClass::Transient {
                retry_after_ms: Some(1500)
            }
        );
        assert_eq!(classify(g, 503, None, None), OutcomeClass::Transient { retry_after_ms: None });
        assert_eq!(classify(g, 401, None, None), OutcomeClass::Auth);
        assert_eq!(classify(g, 403, None, None), OutcomeClass::Auth);
        assert_eq!(classify(g, 400, None, None), OutcomeClass::Payload);
        // 404 e o MODELO que nao existe, nao um payload mau: faz fallback (ver
        // `a_dead_model_falls_back_instead_of_killing_the_chain`).
        assert_eq!(classify(g, 404, None, None), OutcomeClass::ModelNotFound);
        assert_eq!(classify(g, 418, None, None), OutcomeClass::Payload);
    }

    #[test]
    fn backoff_is_deterministic_and_capped() {
        let c = cfg();
        // attempt 0, sem jitter (rng01=0): == base.
        assert_eq!(backoff_ms(0, &c, 0.0, None), 400);
        // attempt 1: base*2 = 800.
        assert_eq!(backoff_ms(1, &c, 0.0, None), 800);
        // jitter maximo (rng01=1): 800 * (1 + 0.25) = 1000.
        assert_eq!(backoff_ms(1, &c, 1.0, None), 1000);
        // cresce mas nunca passa max_delay.
        assert!(backoff_ms(20, &c, 1.0, None) <= c.max_delay_ms);
        // honra o Retry-After do servidor exatamente.
        assert_eq!(backoff_ms(0, &c, 1.0, Some(2000)), 2000);
    }

    #[test]
    fn a_dead_model_falls_back_instead_of_killing_the_chain() {
        // Regressao real: a Google descontinuou o `gemini-2.5-flash-lite` e devolvia 404. Como
        // 404 era Payload ("bug nosso, propaga"), a cadeia MORRIA no primario e o fallback nunca
        // era tentado, apesar de usar um modelo completamente diferente. Um modelo morto num
        // provider nao diz nada sobre o modelo do outro.
        assert_eq!(
            classify(Provider::Gemini, 404, None, None),
            OutcomeClass::ModelNotFound
        );
        let c = cfg();
        let s = LoopState::start();
        assert_eq!(
            plan(&s, &OutcomeClass::ModelNotFound, &c, 0.0),
            Decision::Fallback {
                next: LoopState { provider_index: 1, attempt: 0 }
            }
        );
        // Sem familia seguinte, falha com uma razao QUE SE PERCEBE (nao um "payload invalido"
        // que mandava o utilizador procurar um bug que nao existe).
        let last = LoopState { provider_index: 1, attempt: 0 };
        assert_eq!(
            plan(&last, &OutcomeClass::ModelNotFound, &cfg2(), 0.0),
            Decision::Fail { reason: CoreError::ModelNotFound }
        );
        // Um 400 continua a ser Payload: aí o bug e mesmo nosso e propaga sem mascarar.
        assert_eq!(
            classify(Provider::Gemini, 400, None, None),
            OutcomeClass::Payload
        );
    }

    #[test]
    fn server_retry_after_is_a_floor_never_shortened() {
        // Regressao real (OpenRouter `:free`): o servidor pedia 12s, o `min(max_delay)` encurtava
        // para 8s e o Ember batia-lhe DENTRO do cooldown, garantindo outro 429. O pedido do
        // servidor manda; encurta-lo nunca e uma opcao.
        let c = cfg();
        assert_eq!(backoff_ms(0, &c, 1.0, Some(12_000)), 12_000);
        assert!(backoff_ms(0, &c, 1.0, Some(12_000)) > c.max_delay_ms);
        // Um Retry-After: 0 (o Gemini devolve isto) nao pode virar um hammer instantaneo:
        // pisa-se no base_delay.
        assert_eq!(backoff_ms(0, &c, 0.0, Some(0)), c.base_delay_ms);
    }

    #[test]
    fn transient_asking_longer_than_max_delay_falls_back_instead_of_waiting() {
        // O Ember refina no momento: ninguem espera 29s por um paragrafo. Se o servidor pede
        // mais do que o nosso teto, saltamos ja para a familia seguinte (chave e limites
        // proprios) em vez de dormir ou de re-tentar cedo demais.
        let c = cfg();
        let s = LoopState::start();
        let out = OutcomeClass::Transient {
            retry_after_ms: Some(29_000),
        };
        assert_eq!(
            plan(&s, &out, &c, 0.0),
            Decision::Fallback {
                next: LoopState { provider_index: 1, attempt: 0 }
            }
        );
        // Sem outra familia, falha honestamente em vez de prender o utilizador num sleep longo.
        let last = LoopState { provider_index: 1, attempt: 0 };
        assert_eq!(
            plan(&last, &out, &cfg2(), 0.0),
            Decision::Fail {
                reason: CoreError::AllProvidersFailed
            }
        );
    }

    #[test]
    fn transient_retries_then_falls_back_then_fails() {
        let c = cfg2(); // max_retries_per_provider = 2, provider_count = 2
        let out = OutcomeClass::Transient { retry_after_ms: None };

        // attempt 0 e 1 -> retry no mesmo provider.
        let s0 = LoopState::start();
        match plan(&s0, &out, &c, 0.0) {
            Decision::Retry { next, .. } => {
                assert_eq!(next.provider_index, 0);
                assert_eq!(next.attempt, 1);
            }
            d => panic!("esperava Retry, veio {d:?}"),
        }
        let s_exhausted = LoopState { provider_index: 0, attempt: 2 };
        match plan(&s_exhausted, &out, &c, 0.0) {
            Decision::Fallback { next } => {
                assert_eq!(next.provider_index, 1);
                assert_eq!(next.attempt, 0);
            }
            d => panic!("esperava Fallback, veio {d:?}"),
        }
        // ultimo provider esgotado -> Fail.
        let s_last = LoopState { provider_index: 1, attempt: 2 };
        assert_eq!(
            plan(&s_last, &out, &c, 0.0),
            Decision::Fail { reason: CoreError::AllProvidersFailed }
        );
    }

    #[test]
    fn auth_triggers_fallback_then_fails() {
        let c = cfg2();
        assert!(matches!(
            plan(&LoopState::start(), &OutcomeClass::Auth, &c, 0.0),
            Decision::Fallback { .. }
        ));
        let last = LoopState { provider_index: 1, attempt: 0 };
        assert_eq!(
            plan(&last, &OutcomeClass::Auth, &c, 0.0),
            Decision::Fail { reason: CoreError::Auth }
        );
    }

    #[test]
    fn payload_never_falls_back() {
        let c = cfg();
        assert_eq!(
            plan(&LoopState::start(), &OutcomeClass::Payload, &c, 0.0),
            Decision::Fail { reason: CoreError::Payload }
        );
    }

    #[test]
    fn truncated_falls_back_then_fails_without_retry() {
        let c = cfg2();
        // Nunca faz retry (o corte repetir-se-ia): salta logo para o outro provider.
        assert!(matches!(
            plan(&LoopState::start(), &OutcomeClass::Truncated, &c, 0.0),
            Decision::Fallback { .. }
        ));
        let last = LoopState { provider_index: 1, attempt: 0 };
        assert_eq!(
            plan(&last, &OutcomeClass::Truncated, &c, 0.0),
            Decision::Fail { reason: CoreError::Truncated }
        );
    }

    #[test]
    fn single_provider_transient_exhausts_to_fail() {
        let c = RetryConfig { provider_count: 1, ..RetryConfig::default() };
        let out = OutcomeClass::Transient { retry_after_ms: None };
        // Sem provider seguinte: retries esgotam e falha (nao fica preso nem faz fallback).
        let exhausted = LoopState { provider_index: 0, attempt: c.max_retries_per_provider };
        assert_eq!(
            plan(&exhausted, &out, &c, 0.0),
            Decision::Fail { reason: CoreError::AllProvidersFailed }
        );
    }

    #[test]
    fn backoff_large_attempt_does_not_overflow() {
        let c = cfg();
        // attempt alto (o shift em backoff_ms podia estourar): tem de saturar no teto.
        assert_eq!(backoff_ms(64, &c, 1.0, None), c.max_delay_ms);
        assert_eq!(backoff_ms(1000, &c, 1.0, None), c.max_delay_ms);
    }

    #[test]
    fn content_policy_propagates_by_default_but_can_fall_back() {
        let mut c = cfg();
        assert_eq!(
            plan(&LoopState::start(), &OutcomeClass::ContentPolicy, &c, 0.0),
            Decision::Fail { reason: CoreError::ContentPolicy }
        );
        c.fallback_on_content_policy = true;
        assert!(matches!(
            plan(&LoopState::start(), &OutcomeClass::ContentPolicy, &c, 0.0),
            Decision::Fallback { .. }
        ));
    }

    #[test]
    fn success_succeeds() {
        assert_eq!(
            plan(&LoopState::start(), &OutcomeClass::Success, &cfg(), 0.0),
            Decision::Succeed
        );
    }

    #[test]
    fn three_provider_chain_walks_middle_then_last_on_auth() {
        // provider_count default agora = 3 (Gemini -> OpenAi -> Claude). Auth dispara fallback
        // imediato (sem retry), visitando o do meio e depois o ultimo.
        let c = cfg();
        assert_eq!(c.provider_count, 3);

        // Auth no provider 0 -> fallback para o 1 (o do meio, OpenAi).
        assert!(matches!(
            plan(&LoopState { provider_index: 0, attempt: 0 }, &OutcomeClass::Auth, &c, 0.0),
            Decision::Fallback { next } if next.provider_index == 1
        ));
        // Auth no provider 1 -> fallback para o 2 (ultimo, Claude).
        assert!(matches!(
            plan(&LoopState { provider_index: 1, attempt: 0 }, &OutcomeClass::Auth, &c, 0.0),
            Decision::Fallback { next } if next.provider_index == 2
        ));
        // Auth no ultimo (2) -> Fail (sem mais ninguem).
        assert_eq!(
            plan(&LoopState { provider_index: 2, attempt: 0 }, &OutcomeClass::Auth, &c, 0.0),
            Decision::Fail { reason: CoreError::Auth }
        );
    }

    #[test]
    fn three_provider_chain_exhausts_transient_through_all_three() {
        let c = cfg(); // max_retries_per_provider = 2, provider_count = 3
        let out = OutcomeClass::Transient { retry_after_ms: None };

        // Provider 0 esgota retries -> fallback para o 1.
        let exhausted0 = LoopState { provider_index: 0, attempt: c.max_retries_per_provider };
        assert!(matches!(
            plan(&exhausted0, &out, &c, 0.0),
            Decision::Fallback { next } if next.provider_index == 1
        ));
        // Provider 1 esgota -> fallback para o 2.
        let exhausted1 = LoopState { provider_index: 1, attempt: c.max_retries_per_provider };
        assert!(matches!(
            plan(&exhausted1, &out, &c, 0.0),
            Decision::Fallback { next } if next.provider_index == 2
        ));
        // Provider 2 (ultimo) esgota -> Fail AllProvidersFailed.
        let exhausted2 = LoopState { provider_index: 2, attempt: c.max_retries_per_provider };
        assert_eq!(
            plan(&exhausted2, &out, &c, 0.0),
            Decision::Fail { reason: CoreError::AllProvidersFailed }
        );
    }
}
