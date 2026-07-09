import { m } from "motion/react";

/**
 * O "orb" de refine, agora a marca-estrela em METAMORFOSE junto ao cursor. Conta o refine em
 * movimento: a estrela roda devagar e faz cross-fade continuo entre um estado BRUTO (terracota,
 * ligeiramente menor e opaco-baixo) e um estado POLIDO (ambar com glow, maior e luminoso). Um
 * glow radial respira por baixo. Le-se claramente como "a transformar e a carregar".
 *
 * Tudo compositor-only (opacity + transform: scale/rotate), zero layout/paint por frame, para
 * seguir o cursor aos 120fps sem engasgar. A estrela e um path SVG unico (a marca), escala
 * perfeita a qualquer tamanho. As cores vem dos tokens da marca (globals.css).
 */

// Path da estrela de 4 pontas (a faisca da marca), centrada em 32,32 num viewBox 64.
const STAR = "M32 7 C 34 26 38 30 57 32 C 38 34 34 38 32 57 C 30 38 26 34 7 32 C 26 30 30 26 32 7 Z";

const SIZE = 26;

export function Orb() {
  return (
    <m.div
      className="relative grid place-items-center"
      style={{ width: SIZE, height: SIZE }}
      initial={{ opacity: 0, scale: 0.5 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.5 }}
      transition={{ duration: 0.55, ease: [0.22, 1, 0.36, 1] }}
    >
      {/* Glow radial que respira (quente da marca). So opacidade+escala. */}
      <m.div
        className="absolute"
        style={{
          inset: -8,
          borderRadius: "9999px",
          background:
            "radial-gradient(circle, rgba(253,140,60,0.55) 0%, rgba(253,140,60,0) 70%)",
          filter: "blur(1px)",
          willChange: "opacity, transform",
        }}
        animate={{ opacity: [0.35, 0.9, 0.35], scale: [0.85, 1.2, 0.85] }}
        transition={{ repeat: Infinity, duration: 2.4, ease: [0.4, 0, 0.6, 1] }}
      />

      {/* A estrela roda devagar; as duas camadas (bruto/polido) cross-fadeiam por baixo dela. */}
      <m.div
        className="absolute inset-0"
        style={{ willChange: "transform" }}
        animate={{ rotate: 360 }}
        transition={{ repeat: Infinity, duration: 9, ease: "linear" }}
      >
        {/* Estado BRUTO: terracota, mais pequeno, aparece quando o polido desaparece. */}
        <m.svg
          viewBox="0 0 64 64"
          className="absolute inset-0 h-full w-full"
          style={{ willChange: "opacity, transform" }}
          animate={{ opacity: [1, 0.15, 1], scale: [0.82, 0.82, 0.82] }}
          transition={{ repeat: Infinity, duration: 2.4, ease: [0.4, 0, 0.6, 1] }}
        >
          <path d={STAR} fill="var(--color-ember-raw)" />
        </m.svg>

        {/* Estado POLIDO: ambar luminoso, maior, em contra-fase com o bruto. Sem drop-shadow
            animado (re-rasteriza cada frame); o brilho vem do glow radial por baixo. */}
        <m.svg
          viewBox="0 0 64 64"
          className="absolute inset-0 h-full w-full"
          style={{ willChange: "opacity, transform" }}
          animate={{ opacity: [0.15, 1, 0.15], scale: [1, 1.06, 1] }}
          transition={{ repeat: Infinity, duration: 2.4, ease: [0.4, 0, 0.6, 1] }}
        >
          <path d={STAR} fill="var(--color-ember-glow)" />
        </m.svg>
      </m.div>
    </m.div>
  );
}
