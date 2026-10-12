import { useEffect, useRef, useState } from "react";
import { fmtTime } from "../api";
import { secondsLeft, type OptimizeState, type RaceMark, type RaceSeries } from "./OptimizeProgress";
import { RaceChart } from "./RaceChart";
import { useT } from "../i18n";
import { WorldMap, type WorldPoint } from "./WorldMap";

/** A number of seconds shown as h:mm, sliding smoothly to new values. */
function AnimatedTime({ value }: { value: number }) {
  const [shown, setShown] = useState(value);
  const from = useRef(value);
  useEffect(() => {
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    const start = performance.now();
    const a = from.current;
    if (reduce || !Number.isFinite(a)) {
      setShown(value);
      from.current = value;
      return;
    }
    let frame = 0;
    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / 600);
      const v = a + (value - a) * (1 - (1 - t) ** 3);
      setShown(v);
      from.current = v;
      if (t < 1) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [value]);
  return <>{fmtTime(shown)}</>;
}

/** One line on what the planner is doing. */
function phaseText(state: OptimizeState | null, t: ReturnType<typeof useT>): string {
  if (!state || state.phase === "prepare") {
    return state?.step === "quests" ? t("Chargement des quêtes…") : t("Chargement de la carte…");
  }
  if (state.phase === "construct") return t("Construction des routes de départ…");
  const what =
    state.stage === "qualify"
      ? t("Sélection des meilleurs départs · {done} essayés sur {n}", { done: state.qualified ?? 0, n: state.candidates ?? 0 })
      : state.stage === "final" && state.width === 1
        ? t("Amélioration du meilleur départ par {m} recherches", { m: state.searches ?? 1 })
        : state.stage === "final"
          ? t("Finale : {k} chemins, {m} recherches chacun", { k: state.width ?? 0, m: state.searches ?? 1 })
        : t("Manche {r} sur {n} : {k} chemins, {m} recherches chacun", { r: state.round ?? 1, n: (state.rounds ?? 1) - 1, k: state.width ?? 0, m: state.searches ?? 1 });
  const left = secondsLeft(state);
  // The time left is an estimate: the tries are fixed, their pace depends on the processor.
  return left == null ? what : `${what} · ${t("encore ~{s} s", { s: left })}`;
}

interface Props {
  state: OptimizeState | null;
  series: RaceSeries;
  marks: RaceMark[];
  /** Color slot of each followed route. */
  colors: Record<number, number>;
  route: WorldPoint[];
  log: string[];
  /** End the optimization now and keep the best guide so far. */
  onStop: () => void;
  /** Give the generation up and go back to the configuration. */
  onCancel: () => void;
  /** Stop or cancel asked, the generation not ended yet. */
  interrupting: "stop" | "cancel" | null;
}

export function RunView({ state, series, marks, colors, route, log, onStop, onCancel, interrupting }: Props) {
  const t = useT();
  const [showLog, setShowLog] = useState(false);

  const optimizing = state?.phase === "optimize";
  const ratio = optimizing ? Math.min(1, state.progress ?? 0) : 0;
  // The time of the route kept (best score), the best start before the routes are known.
  const kept = state?.routes?.find((r) => r.id === state.selected)?.time ?? state?.best ?? null;

  return (
    <div className="run-view">
      <header className="run-hero">
        <div>
          <div className="run-phase">{phaseText(state, t)}</div>
          <div className="run-best">{kept != null ? <AnimatedTime value={kept} /> : "—"}</div>
          <div className="run-sub">{kept != null ? t("temps du guide retenu") : t("construction en cours")}</div>
        </div>
        <div className="run-actions">
          {/* Until a first route is built there is no guide to keep: only cancelling. */}
          {optimizing && kept != null && (
            <button className="primary" onClick={onStop} disabled={interrupting !== null} title={t("Arrête l'optimisation et garde le meilleur guide trouvé jusqu'ici")}>
              {interrupting === "stop" ? t("Arrêt…") : t("Arrêter et garder ce guide")}
            </button>
          )}
          <button className="link" onClick={onCancel} disabled={interrupting === "cancel"} title={t("Abandonne la génération et revient à la configuration")}>
            {interrupting === "cancel" ? t("Annulation…") : t("Annuler")}
          </button>
        </div>
      </header>
      <div className={`progress big${optimizing ? "" : " indeterminate"}`} role="progressbar" aria-valuenow={Math.round(ratio * 100)}>
        <div style={optimizing ? { width: `${ratio * 100}%` } : undefined} />
      </div>
      <div className="panel">
        <RaceChart state={state} series={series} marks={marks} colors={colors} />
      </div>
      <div className="panel">
        <WorldMap route={route} live height={520} />
      </div>
      <button className="link" onClick={() => setShowLog(!showLog)}>
        {showLog ? t("Masquer le journal") : t("Afficher le journal")}
      </button>
      {showLog && <pre className="log">{log.join("\n") || t("Démarrage…")}</pre>}
    </div>
  );
}
