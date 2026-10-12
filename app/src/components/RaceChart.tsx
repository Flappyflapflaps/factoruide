import { useRef, useState } from "react";
import { fmtTime } from "../api";
import { useT } from "../i18n";
import type { OptimizeState, RaceMark, RaceRoute, RaceSeries } from "./OptimizeProgress";
import { useWidth } from "./useWidth";

/** Color of a route: its categorical slot, gray without one. */
const color = (slot: number | undefined) => (slot == null ? "var(--text-muted)" : `var(--cat-${slot + 1})`);

/** Value of a step series at time `x` (its last point before), or undefined before it starts. */
function valueAt(points: [number, number][], x: number): number | undefined {
  let v: number | undefined;
  for (const [px, py] of points) {
    if (px > x) break;
    v = py;
  }
  return v;
}

/** About four round tick values between `lo` and `hi` (seconds of play time). */
function ticks(lo: number, hi: number): number[] {
  const steps = [60, 120, 300, 600, 900, 1800, 3600, 7200, 18000];
  const step = steps.find((s) => (hi - lo) / s <= 4) ?? 36000;
  const out = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi; v += step) out.push(v);
  return out;
}

interface Props {
  state: OptimizeState | null;
  series: RaceSeries;
  /** Where each round began. */
  marks: RaceMark[];
  /** Color slot of each route. */
  colors: Record<number, number>;
}

const percent = (share: number) => `${Math.round(share * 100)} %`;

/**
 * The routes the optimization follows, against the share of its tries done. While the best starts
 * are picked, their times are only points on the left edge (they change as better candidates
 * come); once picked, each route's time is drawn, the kept route in bold. After each round the
 * routes left behind stop (gray) and the others go on, each with more searches.
 */
export function RaceChart({ state, series, marks, colors }: Props) {
  const t = useT();
  const plot = useRef<SVGSVGElement>(null);
  const [hover, setHover] = useState<number | null>(null);
  const [frame, W] = useWidth(900);
  const routes: RaceRoute[] = state?.routes ?? [];
  const selected = state?.selected ?? null;
  const qualifying = state?.stage === "qualify";
  const alive = new Set(routes.map((r) => r.id));
  const byTime = [...routes].sort((a, b) => a.time - b.time);

  const legend = (
    <ul className="race-times">
      {byTime.map((r) => (
        <li key={r.id} className={r.id === selected ? "selected" : undefined}>
          <span className="swatch" style={{ background: color(colors[r.id]) }} />
          {r.id === selected ? <strong>{fmtTime(r.time)}</strong> : fmtTime(r.time)}
          {r.id === selected && <span className="tag">{t("retenu")}</span>}
        </li>
      ))}
    </ul>
  );
  const explanation = (
    <p className="race-note">
      {(state?.rounds ?? 1) > 1 && <>{t("Après chaque manche, la meilleure moitié des chemins continue, avec deux fois plus de recherches chacun ; les autres s'arrêtent (en gris).")} </>}
      {t(
        "Le temps en gras est le guide retenu. Ce n'est pas toujours le plus court : chaque chemin est noté sur son temps de jeu, plus un malus pour ce qui rend la montée pénible (farm, allers-retours entre zones, or dépensé en vols). Le guide retenu a la meilleure note : c'est le plus confortable à jouer.",
      )}
    </p>
  );

  if (routes.length === 0) {
    return (
      <figure className="race-chart">
        <div className="empty-chart">{t("Les temps apparaissent dès que les premiers chemins sont construits…")}</div>
      </figure>
    );
  }

  const H = 260;
  const pad = { l: 56, r: 16, t: 12, b: 24 };
  const x0 = state?.race_end ?? 0;
  const x1 = 1;
  const ids = Object.keys(series).map(Number);
  const values = qualifying ? routes.map((r) => r.time) : [...ids.flatMap((id) => series[id].map((p) => p[1])), ...routes.map((r) => r.time)];
  const [lo, hi] = [Math.min(...values), Math.max(...values)];
  const margin = Math.max((hi - lo) * 0.1, 300);
  const [ylo, yhi] = [lo - margin, hi + margin];
  const x = (s: number) => pad.l + ((Math.min(s, x1) - x0) / Math.max(x1 - x0, 1e-9)) * (W - pad.l - pad.r);
  const y = (v: number) => pad.t + ((yhi - v) / (yhi - ylo)) * (H - pad.t - pad.b);
  const now = Math.max(x0, Math.min(state?.progress ?? x0, x1));
  // The routes still running on top of those left behind, the kept route last.
  const rank = (id: number) => (id === selected ? 2 : alive.has(id) ? 1 : 0);
  const drawn = [...ids].sort((a, b) => rank(a) - rank(b));
  const path = (points: [number, number][], running: boolean) =>
    points.map(([px, py], i) => `${i ? `H${x(px).toFixed(1)}V` : `M${x(px).toFixed(1)},`}${y(py).toFixed(1)}`).join("") + (running ? `H${x(now).toFixed(1)}` : "");

  const onMove = (e: React.MouseEvent<SVGSVGElement>) => {
    const box = plot.current?.getBoundingClientRect();
    if (!box || qualifying) return;
    const sx = ((e.clientX - box.left) / box.width) * W;
    const s = x0 + ((sx - pad.l) / (W - pad.l - pad.r)) * (x1 - x0);
    setHover(s >= x0 && s <= now ? s : null);
  };
  // At `hover`, the routes still running then (a route left behind ends at its last point).
  const tip =
    hover == null
      ? []
      : drawn
          .filter((id) => alive.has(id) || (series[id]?.at(-1)?.[0] ?? 0) >= hover)
          .map((id) => ({ id, v: valueAt(series[id] ?? [], hover) }))
          .filter((e): e is { id: number; v: number } => e.v != null)
          .sort((a, b) => a.v - b.v);

  return (
    <figure className="race-chart">
      <figcaption>
        {qualifying ? t("Départ des {n} meilleurs chemins", { n: routes.length }) : t("Temps des chemins au fil de l'optimisation")}
      </figcaption>
      {legend}
      <div className="chart-plot" ref={frame}>
        <svg ref={plot} width={W} height={H} viewBox={`0 0 ${W} ${H}`} role="img" aria-label={byTime.map((r) => fmtTime(r.time)).join(", ")} onMouseMove={onMove} onMouseLeave={() => setHover(null)}>
          {ticks(ylo, yhi).map((v) => (
            <g key={v}>
              <line x1={pad.l} x2={W - pad.r} y1={y(v)} y2={y(v)} className="grid" />
              <text x={pad.l - 6} y={y(v)} textAnchor="end" dominantBaseline="middle" className="axis">
                {fmtTime(v)}
              </text>
            </g>
          ))}
          <text x={pad.l} y={H - 6} className="axis">
            {percent(x0)}
          </text>
          <text x={W - pad.r} y={H - 6} textAnchor="end" className="axis">
            {percent(x1)}
          </text>
          {!qualifying &&
            marks.slice(1).map((m) => (
              <g key={m.round}>
                <line x1={x(m.at)} x2={x(m.at)} y1={pad.t} y2={H - pad.b} className="round-mark" />
                <text x={x(m.at) + 4} y={pad.t + 10} className="axis">
                  {t("{n} chemins", { n: m.width })}
                </text>
              </g>
            ))}
          {qualifying && (
            <text x={(W + pad.l) / 2} y={H / 2} textAnchor="middle" className="axis race-wait">
              {t("Les courbes démarrent quand les {n} meilleurs chemins sont choisis", { n: routes.length })}
            </text>
          )}
          {!qualifying &&
            drawn.map((id) => {
              const points = series[id];
              if (!points?.length) return null;
              const running = alive.has(id);
              return <path key={id} d={path(points, running)} className={`race-line${id === selected ? " selected" : ""}${running ? "" : " out"}`} style={{ stroke: color(running ? colors[id] : undefined) }} />;
            })}
          {routes.map((r) => (
            <circle key={r.id} cx={x(qualifying ? x0 : now)} cy={y(r.time)} r={r.id === selected ? 5 : 4} className="race-dot" style={{ fill: color(colors[r.id]) }}>
              <title>{fmtTime(r.time)}</title>
            </circle>
          ))}
          {hover != null && <line x1={x(hover)} x2={x(hover)} y1={pad.t} y2={H - pad.b} className="crosshair" />}
        </svg>
        {tip.length > 0 && hover != null && (
          <div className="tooltip" style={{ left: `${(x(hover) / W) * 100}%` }}>
            <div className="muted">{t("à {n} % du calcul", { n: Math.round(hover * 100) })}</div>
            {tip.map(({ id, v }) => (
              <div key={id}>
                <span className="swatch" style={{ background: color(alive.has(id) ? colors[id] : undefined) }} /> {id === selected ? <strong>{fmtTime(v)}</strong> : fmtTime(v)}
              </div>
            ))}
          </div>
        )}
      </div>
      {explanation}
    </figure>
  );
}
