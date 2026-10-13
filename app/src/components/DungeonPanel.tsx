import { type DungeonOption } from "../api";
import { useAreaName, useT } from "../i18n";
import { Hint } from "./Hint";

interface Props {
  dungeons: DungeonOption[];
  forced: number[];
  excluded: number[];
  onChange: (forced: number[], excluded: number[]) => void;
}

const levels = (d: DungeonOption) => (d.min_level == null ? "" : d.min_level === d.max_level ? String(d.min_level) : `${d.min_level}–${d.max_level}`);

/** The dungeons by level, a click cycling one: automatic → forced → excluded. */
export function DungeonPanel({ dungeons, forced, excluded, onChange }: Props) {
  const t = useT();
  const areaName = useAreaName();
  const without = (ids: number[], id: number) => ids.filter((x) => x !== id);
  const force = (id: number) => onChange([...without(forced, id), id], without(excluded, id));
  const exclude = (id: number) => onChange(without(forced, id), [...without(excluded, id), id]);
  const remove = (id: number) => onChange(without(forced, id), without(excluded, id));
  const cycle = (id: number) => (forced.includes(id) ? exclude(id) : excluded.includes(id) ? remove(id) : force(id));

  return (
    <section className="panel">
      <h3>
        {t("Donjons")}
        <Hint text={`${t("Cliquez un donjon : forcé → exclu → automatique.")} ${t("Forcé : fait même s'il n'est pas rentable.")}`} />
      </h3>
      <div className="dungeon-grid">
        {dungeons.map((d) => {
          const state = forced.includes(d.id) ? "forced" : excluded.includes(d.id) ? "excluded" : "";
          return (
            <button
              key={d.id}
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
              <small>{levels(d)}</small>
            </button>
          );
        })}
      </div>
    </section>
  );
}
