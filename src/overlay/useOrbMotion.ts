import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useMotionValue, useSpring, useTransform, type MotionValue } from "motion/react";

/** O vetor de "puxao" emitido pelo Rust: quanto o cursor esta a frente da estrela (px fisicos). */
interface OrbMotion {
  vx: number;
  vy: number;
}

/** Valores derivados do movimento do cursor, para a estrela reagir (inclinar + esticar na
 *  direcao). Springs suavizam entre os eventos (o Rust emite adaptativo 30-120fps). */
export interface OrbMotionValues {
  /** Rotacao extra (graus) na direcao do movimento horizontal. */
  tilt: MotionValue<number>;
  /** Escala X (>1 estica na horizontal quando se move na horizontal). */
  stretchX: MotionValue<number>;
  /** Escala Y (>1 estica na vertical). */
  stretchY: MotionValue<number>;
}

/** Escuta o vetor de movimento do Rust e devolve springs prontos a ligar a transforms da estrela.
 *  Tudo compositor-only (rotate/scale). Sem o evento (parado), tudo assenta em repouso. */
export function useOrbMotion(): OrbMotionValues {
  // Valores crus do puxao; springs dao a inercia/assentamento suave.
  const rawX = useMotionValue(0);
  const rawY = useMotionValue(0);
  const sx = useSpring(rawX, { stiffness: 220, damping: 22, mass: 0.6 });
  const sy = useSpring(rawY, { stiffness: 220, damping: 22, mass: 0.6 });

  useEffect(() => {
    const un = listen<OrbMotion>("ember://orb-motion", (e) => {
      // Clampa o puxao para o efeito nao exagerar em saltos grandes do cursor.
      const clamp = (v: number, lim: number) => Math.max(-lim, Math.min(lim, v));
      rawX.set(clamp(e.payload.vx, 60));
      rawY.set(clamp(e.payload.vy, 60));
    });
    return () => {
      void un.then((f) => f());
    };
  }, [rawX, rawY]);

  // Inclina na direcao do movimento horizontal (px -> graus, subtil).
  const tilt = useTransform(sx, [-60, 60], [-14, 14]);
  // Estica na direcao dominante do movimento: mais velocidade -> mais stretch, sem passar de +12%.
  const stretchX = useTransform(sx, (v) => 1 + Math.min(Math.abs(v) / 60, 1) * 0.12);
  const stretchY = useTransform(sy, (v) => 1 + Math.min(Math.abs(v) / 60, 1) * 0.12);

  return { tilt, stretchX, stretchY };
}
