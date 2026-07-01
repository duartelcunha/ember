# Orb: multi-monitor, tamanho, cor e glow (design)

Data: 2026-07-01
Estado: aprovado (brainstorming). Próximo passo: plano de implementação.

## 1. Contexto e objetivo

O orb (`Orb.tsx` + `show_orb_at_cursor`/`orb_follow_loop` em `lib.rs`) segue o cursor
durante o refino. Três problemas a corrigir:

1. **Bug de multi-monitor**: o orb fica preso na borda do ecrã onde apareceu quando o
   cursor passa para outro monitor.
2. **Tamanho**: 26px de diâmetro é grande demais; pedido para ficar mais discreto.
3. **Visual do estado "a pensar"**: hoje é um anel a rodar (arco cónico, tipo spinner de
   loading); pedido um glow pulsante, sem rotação, "seamless e smooth". Cor a passar do
   laranja-ember para o terracota da Claude, só no orb (não é um rebrand da app).

## 2. Decisões aprovadas

| Tema | Decisão |
|---|---|
| Multi-monitor | Clampar ao monitor que contém o **cursor**, não ao monitor atual da janela. |
| Tamanho | 26px → **18px** de diâmetro (`.ember-orb`); caixa exterior e offset ajustam-se. |
| Cor | Variável nova `--color-orb-accent: #D97757` (terracota Claude), usada só no orb. `--color-accent` (Settings/Logo) fica inalterado. |
| Estado "refining" | Anel a rodar **sai**; passa a glow pulsante (box-shadow + scale, `ease-in-out`, loop), sem peças a rodar. |
| Âmbito | Só o orb (`Orb.tsx`, `.ember-orb`/`.ember-ring`, `lib.rs`). `Pill.tsx` e o resto do overlay não mudam. |

## 3. Arquitetura e mudanças

### Rust (`src-tauri/src/lib.rs`)

- **Bug root cause**: `monitor_work_area(w)` usa `w.current_monitor()` — o monitor da
  *janela*, não do cursor. Quando o cursor sai para outro ecrã, `orb_target()` calcula o
  alvo a partir da posição do cursor mas clampa-o aos limites do monitor onde a janela
  **ainda está**, prendendo o orb na borda.
- **Fix**: nova função `monitor_at_point(app, x, y) -> (i32, i32, i32, i32)` que percorre
  `app.available_monitors()` e devolve a área de trabalho do monitor cujo retângulo
  contém o ponto `(x, y)` (posição do cursor). Fallback para `monitor_work_area(w)` se o
  ponto não cair em nenhum monitor (ex.: erro de leitura). `orb_target()` passa a chamar
  esta função com a posição do cursor em vez de `monitor_work_area(w)`.
- **Testável**: a escolha do monitor por ponto é lógica pura (lista de retângulos + ponto
  → retângulo), pode ir para `ember_core::selection` ao lado do `clamp_pos` já existente,
  com testes sem SO (ex.: ponto no segundo monitor de dois lado-a-lado, ponto na
  fronteira, ponto fora de todos os monitores → fallback).
- **`ORB_OFFSET`**: mantém-se conceptualmente (offset do orb face ao cursor); valor em px
  físicos ajusta-se proporcionalmente à nova caixa exterior do orb (ver secção 4).

### Frontend (`src/overlay/Orb.tsx` + `src/styles/globals.css`)

- `.ember-orb`: `width`/`height` 26px → 18px.
- Caixa exterior do componente (`style={{ width: 40, height: 40 }}` em `Orb.tsx`): reduz
  proporcionalmente (ronda os 28px, o suficiente para o glow "respirar" sem cortar).
- `.ember-ring`: deixa de ter `conic-gradient` + `animate={{ rotate: 360 }}`. É substituído
  por um glow: `box-shadow` a animar entre um valor mais subtil e um mais intenso (raio e
  opacidade), em `keyframes`/`animate` do Motion, `duration` ~1.6s, `ease: "easeInOut"`,
  `repeat: Infinity`, sincronizado com o `scale` que já existe no `.ember-orb` (que se
  mantém, ligeiramente ajustado em amplitude para não parecer excessivo numa bola menor).
- Nova variável CSS `--color-orb-accent: #D97757` em `@theme` (ao lado de
  `--color-accent`, sem o substituir). `.ember-orb`/`.ember-ring`/o glow passam a usar
  `--color-orb-accent` em vez de `--color-accent`. Nenhum outro componente (Settings,
  Logo, Pill) muda de cor.

## 4. Detalhe de dimensões

| Elemento | Antes | Depois |
|---|---|---|
| `.ember-orb` (bola) | 26px | 18px |
| Caixa exterior (`Orb.tsx` style) | 40x40 | ~28x28 |
| `ORB_OFFSET` (lib.rs) | 26px | ajustar para manter o offset visual face ao cursor (proporcional à redução da caixa) |

Valores exatos de glow (raio/opacidade do box-shadow) afinam-se a olho durante a
implementação, correndo o `run` skill; não há um número "certo" a priori.

## 5. Testes

- `ember-core::selection`: testes para a escolha de monitor por ponto — ponto dentro do
  monitor 1, ponto dentro do monitor 2 (dois monitores lado a lado), ponto na fronteira,
  ponto fora de todos (fallback). Sem SO/rede, como os testes de `clamp_pos` já existentes.
- Verificação manual (skill `run`): com dois monitores (ou um monitor + simulação de
  posição), confirmar que o orb atravessa para o segundo ecrã a seguir ao cursor; olhar
  para o tamanho/cor/glow em uso real durante um refino.

## 6. Fora de âmbito (YAGNI)

- Mudar a cor de `Pill.tsx` (success/error/hint) ou de qualquer outra UI — só o orb.
- Rebrand de `--color-accent` (Settings, Logo, botões).
- Suporte a mais de dois monitores em simultâneo além do necessário para o fix (a lógica
  de "ponto dentro de retângulo" já escala para N monitores sem trabalho extra).
