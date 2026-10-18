import type { Options, Params } from "../api";
import { useT } from "../i18n";
import { SettingRow } from "./ParamField";

type Point = [number, number];

/** The curve's value at `level`: straight lines between its points, flat beyond them. */
function at(curve: Point[], level: number): number {
  if (!curve.length) return 0;
  const i = curve.findIndex(([l]) => l >= level);
  if (i === 0) return curve[0][1];
  if (i < 0) return curve[curve.length - 1][1];
  const [[x0, y0], [x1, y1]] = [curve[i - 1], curve[i]];
  return y0 + ((y1 - y0) * (level - x0)) / (x1 - x0);
}

/**
 * The class's power curve by level (how it fares against the average class, in levels: what the
 * guide has it take on) and how much its gear counts, as the planner has them for the class,
 * each point adjustable. Back to the class's values once every point is at its default.
 */
export function PowerCurve({ params, options, className, maxLevel, onChange }: { params: Params; options: Options; className: string; maxLevel: number; onChange: (curve: Point[], gear: number | null) => void }) {
  const t = useT();
  const own = options.class_power?.[className] ?? { curve: [], gear: 1 };
  const curve = params.power_curve?.length ? params.power_curve : own.curve;
  const gear = params.power_gear ?? own.gear;
  const setPoint = (k: number, value: number) => {
    const next = curve.map((p, i): Point => (i === k ? [p[0], value] : p));
    const same = next.length === own.curve.length && next.every(([l, v]) => Math.abs(at(own.curve, l) - v) < 1e-9);
    onChange(same ? [] : next, params.power_gear ?? null);
  };
  // The curve drawn from level 1 to the cap, -3 to +3 levels.
  const [w, h] = [240, 60];
  const x = (l: number) => ((l - 1) / Math.max(1, maxLevel - 1)) * w;
  const y = (v: number) => h / 2 - (Math.max(-3, Math.min(3, v)) / 3) * (h / 2);
  const line = (c: Point[]) => Array.from({ length: maxLevel }, (_, i) => `${x(i + 1).toFixed(1)},${y(at(c, i + 1)).toFixed(1)}`).join(" ");
  return (
    <div className="power-curve">
      <p className="power-curve-help">
        {t("Courbe de puissance de la classe : ce qu'elle affronte à chaque niveau par rapport à une classe moyenne, en niveaux (+1 = comme un personnage d'un niveau de plus). Le guide n'en tue pas plus vite, il prend des quêtes et des mobs plus ou moins hauts.")}
      </p>
      <svg className="power-curve-graph" viewBox={`0 0 ${w} ${h}`} role="img" aria-label={t("Courbe de puissance de la classe")}>
        <line x1={0} x2={w} y1={h / 2} y2={h / 2} className="axis" />
        {curve !== own.curve && <polyline points={line(own.curve)} className="default" />}
        <polyline points={line(curve)} className="current" />
      </svg>
      {curve.map(([level, value], k) => (
        <SettingRow
          key={level}
          kind="number"
          label={t("Niveau {n}", { n: level })}
          value={value}
          initial={at(own.curve, level)}
          min={-3}
          max={3}
          step={0.5}
          unit="niveaux"
          onChange={(v) => setPoint(k, v as number)}
        />
      ))}
      <SettingRow
        kind="number"
        label="Dépendance à l'équipement"
        help="Combien l'équipement compte pour la classe (1 = moyenne) : un équipement de base retient le personnage jusqu'à 2 niveaux en fin de montée, un très bon le pousse d'un niveau."
        value={gear}
        initial={own.gear}
        min={0}
        max={2}
        step={0.1}
        unit="×"
        onChange={(v) => onChange(params.power_curve ?? [], Math.abs((v as number) - own.gear) < 1e-9 ? null : (v as number))}
      />
    </div>
  );
}
