import { m } from "motion/react";
import { useOrbMotion } from "./useOrbMotion";

/**
 * O "orb" de refine: metamorfose de FORMA por cross-fade entre duas silhuetas genuinamente
 * diferentes, um blob rugoso e irregular (BRUTO) e uma estrela de 4 pontas afiada (POLIDO). Em
 * loop, o bruto encolhe/esbate enquanto a estrela cresce/acende e vice-versa, com o glow a
 * brilhar mais no pico polido. Le-se como algo a cristalizar do caos: o refine.
 *
 * NAO anima o atributo `d` (o preset `domAnimation` do overlay nao o suporta). Em vez disso,
 * cross-fade de opacity + scale entre duas <path> sobrepostas: compositor-only, funciona com o
 * preset leve, e a diferenca real de forma entre as duas faz a leitura de metamorfose. Reage
 * ao movimento do rato (inclina + estica, via ember://orb-motion).
 */

// BRUTO: blob assimetrico, pontas curtas e gastas, cantos irregulares.
const RAW =
  "M31 15 C 36 27 37 28 48 30 C 38 33 36 34 33 45 C 30 36 28 35 17 33 C 27 30 26 29 31 15 Z";
// POLIDO: estrela de 4 pontas nitida e afiada, pontas longas.
const POLISHED =
  "M32 4 C 34 27 37 30 60 32 C 37 34 34 37 32 60 C 30 37 27 34 4 32 C 27 30 30 27 32 4 Z";

const SIZE = 28;

// Ciclo lento e premium; bruto e polido demoram-se no seu pico, a transicao e suave.
const CYCLE = { repeat: Infinity, duration: 3.4, ease: [0.45, 0, 0.55, 1] as const };

export function Orb() {
  const { tilt, stretchX, stretchY } = useOrbMotion();
  return (
    <m.div
      className="relative grid place-items-center"
      style={{ width: SIZE, height: SIZE }}
      initial={{ opacity: 0, scale: 0.5 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.5 }}
      transition={{ duration: 0.55, ease: [0.22, 1, 0.36, 1] }}
    >
      {/* Glow radial, mais forte no pico polido (quando cristaliza). */}
      <m.div
        className="absolute"
        style={{
          inset: -9,
          borderRadius: "9999px",
          background:
            "radial-gradient(circle, rgba(253,140,60,0.6) 0%, rgba(253,140,60,0) 70%)",
          filter: "blur(1px)",
          willChange: "opacity, transform",
        }}
        animate={{ opacity: [0.28, 0.9, 0.28], scale: [0.78, 1.2, 0.78] }}
        transition={CYCLE}
      />

      {/* Reacao ao cursor: inclina + estica na direcao do movimento (springs). */}
      <m.div
        className="absolute inset-0"
        style={{ rotate: tilt, scaleX: stretchX, scaleY: stretchY, willChange: "transform" }}
      >
        {/* Rotacao lenta continua, para nunca parecer estatica. */}
        <m.div
          className="absolute inset-0"
          style={{ willChange: "transform" }}
          animate={{ rotate: 360 }}
          transition={{ repeat: Infinity, duration: 14, ease: "linear" }}
        >
          {/* Cada estado numa camada <div> propria: o scale/opacity vao no div (transform-origin
              ao centro, fiavel), nao no <path> (onde o transform-origin do SVG e traicoeiro). */}
          {/* BRUTO: blob terracota, encolhe e esbate quando a estrela toma conta. */}
          <m.div
            className="absolute inset-0"
            style={{ willChange: "opacity, transform" }}
            animate={{ opacity: [1, 0, 1], scale: [0.9, 0.68, 0.9] }}
            transition={CYCLE}
          >
            <svg viewBox="0 0 64 64" className="h-full w-full">
              <path d={RAW} fill="var(--color-ember-raw)" />
            </svg>
          </m.div>
          {/* POLIDO: estrela ambar afiada, cresce e acende em contra-fase com o bruto. */}
          <m.div
            className="absolute inset-0"
            style={{ willChange: "opacity, transform" }}
            animate={{ opacity: [0, 1, 0], scale: [0.85, 1.06, 0.85] }}
            transition={CYCLE}
          >
            <svg viewBox="0 0 64 64" className="h-full w-full">
              <path d={POLISHED} fill="var(--color-ember-glow)" />
            </svg>
          </m.div>
        </m.div>
      </m.div>
    </m.div>
  );
}
