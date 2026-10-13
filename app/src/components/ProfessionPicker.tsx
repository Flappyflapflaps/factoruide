import { useEffect, useRef, useState } from "react";
import type { ProfessionGoal } from "../api";
import { useT } from "../i18n";

/**
 * Mirror of `fg_route::profession::PROFESSIONS`: default skill at the levels of `MILESTONE_LEVELS`
 * (from classic leveling guides: taken around level 5, slow start, catching up by level 20;
 * gathering ahead, crafting behind its materials, fishing fast then slow).
 */
export const PROFESSIONS: { key: string; label: string; secondary: boolean; curve: number[] }[] = [
  { key: "alchemy", label: "Alchimie", secondary: false, curve: [25, 70, 115, 145, 170, 215, 265, 300] },
  { key: "blacksmithing", label: "Forge", secondary: false, curve: [20, 55, 100, 125, 150, 205, 260, 300] },
  { key: "enchanting", label: "Enchantement", secondary: false, curve: [5, 35, 75, 110, 140, 195, 250, 300] },
  { key: "engineering", label: "Ingénierie", secondary: false, curve: [20, 60, 100, 125, 150, 200, 255, 300] },
  { key: "herbalism", label: "Herboristerie", secondary: false, curve: [40, 90, 135, 165, 190, 240, 285, 300] },
  { key: "leatherworking", label: "Travail du cuir", secondary: false, curve: [20, 60, 105, 130, 150, 200, 250, 300] },
  { key: "mining", label: "Minage", secondary: false, curve: [30, 75, 115, 145, 170, 225, 275, 300] },
  { key: "skinning", label: "Dépeçage", secondary: false, curve: [35, 75, 110, 135, 160, 210, 260, 300] },
  { key: "tailoring", label: "Couture", secondary: false, curve: [20, 60, 105, 135, 160, 215, 265, 300] },
  { key: "cooking", label: "Cuisine", secondary: true, curve: [35, 80, 125, 150, 175, 225, 265, 300] },
  { key: "fishing", label: "Pêche", secondary: true, curve: [45, 95, 140, 165, 190, 230, 265, 300] },
  { key: "first_aid", label: "Secourisme", secondary: true, curve: [30, 75, 120, 150, 180, 240, 285, 300] },
];

const MILESTONE_LEVELS = [10, 15, 20, 25, 30, 40, 50, 60];

/** Mirror of `fg_route::profession::START_LEVEL`. */
const START_LEVEL = 5;

function defaultMilestones(key: string): [number, number][] {
  const curve = PROFESSIONS.find((p) => p.key === key)?.curve ?? [];
  return curve.map((s, i) => [MILESTONE_LEVELS[i], s]);
}

/** Mirror of `fg_route::profession::target_at`: skill to have at a level (0 until the profession is taken). */
function skillAt(goal: ProfessionGoal, level: number): number {
  const start = goal.start_level ?? START_LEVEL;
  if (level <= start) return 0;
  const sorted = [...(goal.milestones ?? [])].sort((a, b) => a[0] - b[0]);
  if (sorted.length === 0) sorted.push(...defaultMilestones(goal.key));
  const points = sorted.filter(([l]) => l > start);
  let curve = 5 * (level - start);
  if (sorted.length > 0) {
    const i = points.findIndex(([l]) => l >= level);
    if (i === 0)
      curve = Math.trunc((points[0][1] * (level - start)) / Math.max(1, points[0][0] - start));
    else if (i > 0) {
      const [[l0, s0], [l1, s1]] = [points[i - 1], points[i]];
      curve =
        s0 + Math.trunc(((s1 - s0) * (level - l0)) / Math.max(1, l1 - l0));
    } else curve = sorted[sorted.length - 1][1];
  }
  return Math.max(0, Math.min(curve, goal.target, 300));
}

interface Props {
  goals: ProfessionGoal[];
  onChange: (goals: ProfessionGoal[]) => void;
}

/** Plot of the skill (0-300) by character level (0-60). */
const W = 296;
const H = 150;
const M = { left: 30, right: 10, top: 12, bottom: 20 };
const X = (level: number) => M.left + (level / 60) * (W - M.left - M.right);
const Y = (skill: number) => H - M.bottom - (skill / 300) * (H - M.top - M.bottom);
const snap = (v: number, step: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, Math.round(v / step) * step));

/**
 * The skill to have by level, as a curve: each milestone dragged up or down (steps of 5), the
 * level the profession is taken dragged along the axis; arrows move the point in focus.
 */
function GoalCurve({ goal, onChange }: { goal: ProfessionGoal; onChange: (patch: Partial<ProfessionGoal>) => void }) {
  const t = useT();
  const svg = useRef<SVGSVGElement>(null);
  const [drag, setDrag] = useState<number | "start" | null>(null);
  const [hover, setHover] = useState<number | "start" | null>(null);
  const start = goal.start_level ?? START_LEVEL;
  const setSkill = (level: number, skill: number) =>
    onChange({ milestones: MILESTONE_LEVELS.map((l) => [l, l === level ? skill : skillAt(goal, l)]) });
  const toPlot = (e: React.PointerEvent) => {
    const ctm = svg.current?.getScreenCTM();
    if (!ctm) return null;
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(ctm.inverse());
    return { level: ((p.x - M.left) / (W - M.left - M.right)) * 60, skill: ((H - M.bottom - p.y) / (H - M.top - M.bottom)) * 300 };
  };
  const move = (e: React.PointerEvent) => {
    if (drag == null) return;
    const p = toPlot(e);
    if (!p) return;
    if (drag === "start") onChange({ start_level: snap(p.level, 1, 1, 59) });
    else setSkill(drag, snap(p.skill, 5, 0, 300));
  };
  const line = Array.from({ length: 61 }, (_, l) => `${X(l).toFixed(1)},${Y(skillAt(goal, l)).toFixed(1)}`).join(" ");
  const shown = drag ?? hover;
  const label =
    shown === "start"
      ? { x: X(start), y: Y(0), text: t("dès le niveau") + " " + start }
      : shown != null
        ? { x: X(shown), y: Y(skillAt(goal, shown)), text: `${t("niv. {level}", { level: shown })} · ${skillAt(goal, shown)}` }
        : null;

  return (
    <svg
      ref={svg}
      className="goal-curve"
      viewBox={`0 0 ${W} ${H}`}
      width={W}
      height={H}
      onPointerMove={move}
      onPointerUp={() => setDrag(null)}
      onPointerCancel={() => setDrag(null)}
    >
      {[0, 100, 200, 300].map((v) => (
        <g key={v}>
          <line className="grid" x1={M.left} x2={W - M.right} y1={Y(v)} y2={Y(v)} />
          <text className="axis" x={M.left - 6} y={Y(v)} textAnchor="end" dominantBaseline="middle">
            {v}
          </text>
        </g>
      ))}
      {[10, 20, 30, 40, 50, 60].map((l) => (
        <text key={l} className="axis" x={X(l)} y={H - 4} textAnchor="middle">
          {l}
        </text>
      ))}
      <polyline className="series-line" points={line} />
      {/* The level the profession is taken, on the axis. */}
      <circle
        className={`goal-start${shown === "start" ? " active" : ""}`}
        cx={X(start)}
        cy={Y(0)}
        r={5}
        tabIndex={0}
        aria-label={t("dès le niveau") + " " + start}
        onPointerDown={(e) => {
          (e.target as Element).setPointerCapture?.(e.pointerId);
          setDrag("start");
        }}
        onPointerEnter={() => setHover("start")}
        onPointerLeave={() => setHover(null)}
        onKeyDown={(e) => {
          const d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
          if (!d) return;
          e.preventDefault();
          onChange({ start_level: snap(start + d, 1, 1, 59) });
        }}
      />
      {MILESTONE_LEVELS.filter((l) => l > start).map((l) => {
        const v = skillAt(goal, l);
        return (
          <g key={l}>
            {/* A larger, invisible target to grab the point. */}
            <circle
              className="goal-hit"
              cx={X(l)}
              cy={Y(v)}
              r={11}
              tabIndex={0}
              aria-label={`${t("niv. {level}", { level: l })} · ${v}`}
              onPointerDown={(e) => {
                (e.target as Element).setPointerCapture?.(e.pointerId);
                setDrag(l);
              }}
              onPointerEnter={() => setHover(l)}
              onPointerLeave={() => setHover(null)}
              onFocus={() => setHover(l)}
              onBlur={() => setHover(null)}
              onKeyDown={(e) => {
                const d = e.key === "ArrowUp" ? 5 : e.key === "ArrowDown" ? -5 : 0;
                if (!d) return;
                e.preventDefault();
                setSkill(l, snap(v + d, 5, 0, 300));
              }}
            />
            <circle className={`series-dot goal-dot${shown === l ? " active" : ""}`} cx={X(l)} cy={Y(v)} r={4.5} />
          </g>
        );
      })}
      {label && (
        <text className="goal-label" x={Math.min(Math.max(label.x, M.left + 40), W - 40)} y={Math.max(label.y - 10, 10)} textAnchor="middle">
          {label.text}
        </text>
      )}
    </svg>
  );
}

export function ProfessionPicker({ goals, onChange }: Props) {
  const t = useT();
  const secondary = (key: string) => Boolean(PROFESSIONS.find((p) => p.key === key)?.secondary);
  const label = (key: string) => t(PROFESSIONS.find((p) => p.key === key)?.label ?? key);
  // The two primary professions, in their slots; the secondary ones after them.
  const primaries = goals.filter((g) => !secondary(g.key));
  const slots = [primaries[0] ?? null, primaries[1] ?? null];
  const secondaries = goals.filter((g) => secondary(g.key));
  const setSlot = (i: number, key: string) => {
    const next = slots.map((g, j) => (j !== i ? g : !key ? null : g?.key === key ? g : { key, target: 300 }));
    onChange([...next.filter((g): g is ProfessionGoal => g !== null), ...secondaries]);
  };
  const toggle = (key: string, on: boolean) => onChange(on ? [...goals, { key, target: 300 }] : goals.filter((g) => g.key !== key));
  const update = (key: string, patch: Partial<ProfessionGoal>) => onChange(goals.map((g) => (g.key === key ? { ...g, ...patch } : g)));

  // The goal of one profession at a time, in a bubble under its ⚙ (closed by a click elsewhere).
  const [open, setOpen] = useState<string | null>(null);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(null);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  /** ⚙ beside a profession taken: the skill to reach, from which level, and the skill by level. */
  const goalButton = (goal: ProfessionGoal) => {
    const custom = goal.target !== 300 || (goal.start_level ?? START_LEVEL) !== START_LEVEL || Boolean(goal.milestones?.length);
    return (
      <div className="goal" ref={open === goal.key ? box : undefined}>
        <button
          type="button"
          className={`icon goal-button${custom ? " custom" : ""}${open === goal.key ? " active" : ""}`}
          title={t("Compétence à atteindre selon le niveau du personnage")}
          onClick={(e) => {
            e.preventDefault();
            setOpen(open === goal.key ? null : goal.key);
          }}
        >
          ⚙
        </button>
        {open === goal.key && (
          <div className="goal-pop">
            <strong>{label(goal.key)}</strong>
            <div className="goal-row">
              <label>
                {t("jusqu'à")}
                <input type="number" min={1} max={300} step={5} value={goal.target} onChange={(e) => update(goal.key, { target: Number(e.target.value) })} />
              </label>
              <label title={t("Niveau où vous prenez le métier : la courbe part de 0 à ce niveau")}>
                {t("dès le niveau")}
                <input type="number" min={1} max={60} step={1} value={goal.start_level ?? START_LEVEL} onChange={(e) => update(goal.key, { start_level: Number(e.target.value) })} />
              </label>
            </div>
            <span className="muted">
              {t("Paliers")} {goal.milestones?.length ? t("(personnalisés)") : t("(guides classiques)")}
            </span>
            <GoalCurve goal={goal} onChange={(patch) => update(goal.key, patch)} />
            {custom && (
              <button type="button" className="link" onClick={() => update(goal.key, { target: 300, start_level: undefined, milestones: [] })}>
                {t("Revenir aux valeurs des guides")}
              </button>
            )}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="professions">
      <span className="muted">{t("Principaux (2 max)")}</span>
      {slots.map((goal, i) => (
        <div className="profession-slot" key={i}>
          <select aria-label={t("Principaux (2 max)")} value={goal?.key ?? ""} onChange={(e) => setSlot(i, e.target.value)}>
            <option value="">{t("Aucun")}</option>
            {PROFESSIONS.filter((p) => !p.secondary && p.key !== slots[1 - i]?.key).map((p) => (
              <option key={p.key} value={p.key}>
                {t(p.label)}
              </option>
            ))}
          </select>
          {goal && goalButton(goal)}
        </div>
      ))}
      <span className="muted secondary-label">{t("Secondaires")}</span>
      <div className="profession-secondary">
        {PROFESSIONS.filter((p) => p.secondary).map((p) => {
          const goal = secondaries.find((g) => g.key === p.key);
          return (
            <div className="profession-check" key={p.key}>
              <label className="field-bool">
                <input type="checkbox" checked={Boolean(goal)} onChange={(e) => toggle(p.key, e.target.checked)} />
                <span>{t(p.label)}</span>
              </label>
              {goal && goalButton(goal)}
            </div>
          );
        })}
      </div>
    </div>
  );
}
