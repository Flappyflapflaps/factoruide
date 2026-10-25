import { type DungeonOption } from "../api";
import { useAreaName, useT } from "../i18n";
import { Hint } from "./Hint";

interface Props {
  dungeons: DungeonOption[];
  forced: number[];
  excluded: number[];
  /** The player's own level for forced dungeons: (area, level). */
  levels: [number, number][];
  onChange: (forced: number[], excluded: number[], levels: [number, number][]) => void;
}

const levels = (d: DungeonOption) => (d.min_level == null ? "" : d.min_level === d.max_level ? String(d.min_level) : `${d.min_level}–${d.max_level}`);

/** Levels offered for a dungeon: a few around those of its quests. */
const choices = (d: DungeonOption) => {
  const from = Math.max(1, (d.min_level ?? 1) - 4);
  const to = Math.max(from, (d.max_level ?? d.min_level ?? 1) + 4);
  return Array.from({ length: to - from + 1 }, (_, i) => from + i);
};

/** The dungeons by level, a click cycling one: automatic → forced → excluded. A forced one can
 *  be given the player's own level (a group met at that level). */
export function DungeonPanel({ dungeons, forced, excluded, levels: chosen, onChange }: Props) {
  const t = useT();
  const areaName = useAreaName();
  const without = (ids: number[], id: number) => ids.filter((x) => x !== id);
  // A level is kept for forced dungeons only.
  const unlevel = (id: number) => chosen.filter(([area]) => area !== id);
  const force = (id: number) => onChange([...without(forced, id), id], without(excluded, id), chosen);
  const exclude = (id: number) => onChange(without(forced, id), [...without(excluded, id), id], unlevel(id));
  const remove = (id: number) => onChange(without(forced, id), without(excluded, id), unlevel(id));
  const cycle = (id: number) => (forced.includes(id) ? exclude(id) : excluded.includes(id) ? remove(id) : force(id));
  const setLevel = (id: number, level: number | null) => onChange(forced, excluded, level == null ? unlevel(id) : [...unlevel(id), [id, level]]);

  return (
    <section className="panel">
      <h3>
        {t("Donjons")}
        <Hint
          text={`${t("Cliquez un donjon : forcé → exclu → automatique.")} ${t("Forcé : fait même s'il n'est pas rentable.")} ${t("Un donjon forcé peut se faire à votre niveau : choisissez-le sous son nom.")}`}
        />
      </h3>
      <div className="dungeon-grid">
        {dungeons.map((d) => {
          const state = forced.includes(d.id) ? "forced" : excluded.includes(d.id) ? "excluded" : "";
          const own = chosen.find(([area]) => area === d.id)?.[1];
          return (
            <div key={d.id} className="dungeon-cell">
              <button
                className={`dungeon ${state}`}
                onClick={() => cycle(d.id)}
                title={state === "forced" ? t("Forcé") : state === "excluded" ? t("Exclu") : t("Automatique")}
                aria-label={`${areaName(d)} ${levels(d)} : ${state === "forced" ? t("Forcé") : state === "excluded" ? t("Exclu") : t("Automatique")}`}
              >
                <span>
                  {state === "forced" && <b aria-hidden>★ </b>}
                  {state === "excluded" && <b aria-hidden>✕ </b>}
                  {areaName(d)}
                </span>
                <small>{own != null ? String(own) : levels(d)}</small>
              </button>
              {state === "forced" && (
                <select
                  className="dungeon-level"
                  value={own ?? ""}
                  aria-label={`${areaName(d)} : ${t("Niveau du donjon")}`}
                  title={t("Niveau du donjon")}
                  onChange={(e) => setLevel(d.id, e.target.value === "" ? null : Number(e.target.value))}
                >
                  <option value="">{`${t("Niveau du donjon")} : ${t("Automatique")} (${levels(d)})`}</option>
                  {choices(d).map((l) => (
                    <option key={l} value={l}>
                      {t("niveau {n}", { n: l })}
                    </option>
                  ))}
                </select>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
