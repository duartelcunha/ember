import { m } from "motion/react";
import { useOrbMotion } from "./useOrbMotion";

/**
 * O "orb" de refine: A MARCA em movimento. E sempre a estrela de 4 pontas do logo (reconhecivel),
 * preenchida com o mesmo gradiente diagonal do logo, terracota BRUTA no canto inferior-esquerdo,
 * a afinar-se para AMBAR polido no superior-direito. O "refine" e um brilho que VARRE a diagonal,
 * do canto bruto ao polido, em loop: le-se como a estrela a ser continuamente lapidada/refinada.
 * Um glow subtil respira por baixo. Roda devagar e reage ao cursor (inclina + estica).
 *
 * Tudo compositor-only (opacity + transform); o sweep e um <rect> com mask da estrela a
 * transl-dar na diagonal, sem animar filtros nem paths (o preset leve do overlay chega).
 */

// A estrela de 4 pontas da marca, viewBox 64, centrada em 32,32.
const STAR = "M32 4 C 34 27 37 30 60 32 C 37 34 34 37 32 60 C 30 37 27 34 4 32 C 27 30 30 27 32 4 Z";

const SIZE = 28;

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
      {/* Glow radial subtil (minimalista, contido para nao ser cortado pela borda da janela). */}
      <m.div
        className="absolute"
        style={{
          inset: -4,
          borderRadius: "9999px",
          background:
            "radial-gradient(circle, rgba(253,140,60,0.5) 0%, rgba(253,140,60,0) 72%)",
          filter: "blur(0.5px)",
          willChange: "opacity, transform",
        }}
        animate={{ opacity: [0.22, 0.5, 0.22], scale: [0.92, 1.04, 0.92] }}
        transition={{ repeat: Infinity, duration: 2.6, ease: [0.4, 0, 0.6, 1] }}
      />

      {/* Reacao ao cursor: inclina + estica na direcao do movimento (springs). */}
      <m.div
        className="absolute inset-0"
        style={{ rotate: tilt, scaleX: stretchX, scaleY: stretchY, willChange: "transform" }}
      >
        {/* Rotacao lenta continua, para a marca nunca parecer estatica. */}
        <m.div
          className="absolute inset-0"
          style={{ willChange: "transform" }}
          animate={{ rotate: 360 }}
          transition={{ repeat: Infinity, duration: 16, ease: "linear" }}
        >
          <svg viewBox="0 0 64 64" className="absolute inset-0 h-full w-full">
            <defs>
              {/* Gradiente diagonal do logo: terracota bruta (baixo-esq) -> ambar polido (cima-dir). */}
              <linearGradient id="ember-diag" x1="0.1" y1="0.9" x2="0.9" y2="0.1">
                <stop offset="0%" stopColor="var(--color-ember-raw)" />
                <stop offset="55%" stopColor="var(--color-accent)" />
                <stop offset="100%" stopColor="var(--color-ember-glow)" />
              </linearGradient>
              {/* Mask com a forma da estrela: tudo o que desenharmos so aparece dentro dela. */}
              <mask id="ember-star">
                <path d={STAR} fill="#fff" />
              </mask>
            </defs>

            {/* Base: a estrela da marca com o gradiente bruto->polido. */}
            <path d={STAR} fill="url(#ember-diag)" />

            {/* Sweep de refino: uma banda de luz que varre a diagonal (do canto bruto ao polido),
                recortada pela forma da estrela. Da a leitura de "a ser refinada" em loop. Move-se
                por `translateX` (transform, compositor-safe), nao pelo atributo `x` (que o preset
                leve do overlay pode nao animar). A rotacao -45deg faz o varrimento diagonal. */}
            <g mask="url(#ember-star)">
              <m.rect
                x={0}
                y={-16}
                width={22}
                height={96}
                fill="rgba(255,240,210,0.9)"
                style={{
                  transformBox: "view-box",
                  transformOrigin: "32px 32px",
                  willChange: "transform",
                }}
                initial={{ rotate: -45, x: -60 }}
                animate={{ x: [-60, 74] }}
                transition={{
                  repeat: Infinity,
                  duration: 2.2,
                  ease: [0.5, 0, 0.5, 1],
                  repeatDelay: 0.6,
                }}
              />
            </g>
          </svg>
        </m.div>
      </m.div>
    </m.div>
  );
}
