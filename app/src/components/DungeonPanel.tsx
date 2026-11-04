import { type DungeonOption } from "../api";
import { useAreaName, useT } from "../i18n";
import { Hint } from "./Hint";

/** A dungeon's choice: automatic, forced, forced with all its quests, or excluded. */
export interface DungeonChoices {
  forced: number[];
  forcedQuests: number[];
  excluded: number[];
  /** The player's own level for forced dungeons: (area, level). */
  levels: [number, number][];
}

interface Props {
  dungeons: DungeonOption[];
  choices: DungeonChoices;
  onChange: (choices: DungeonChoices) => void;
  /** Levels dungeons are run at, around their level, and the game's highest level. */
  from: number;
  to: number;
  maxLevel: number;
  onLevels: (from: number, to: number) => void;
}

/** Levels offered for a forced dungeon's own level: a few around those it is run at. */
const ownChoices = (range: [number, number] | null, maxLevel: number) => {
  const from = Math.max(1, (range?.[0] ?? 1) - 4);
  const to = Math.min(maxLevel, Math.max(from, (range?.[1] ?? from) + 4));
  return Array.from({ length: to - from + 1 }, (_, i) => from + i);
};

/** The levels a dungeon of level `level` is run at: from `from` to `to` around it, within the
 * game's levels, at the highest one when the window is above them (null: none). */
export function dungeonLevels(level: number, from: number, to: number, maxLevel: number): [number, number] | null {
  const [low, high] = [Math.min(maxLevel, Math.max(1, level + from)), Math.min(maxLevel, level + to)];
  return low <= high ? [low, high] : null;
}

/** The dungeons whose levels the choice leaves none for (empty: the choice works). */
export function dungeonsWithoutLevels(dungeons: DungeonOption[], from: number, to: number, maxLevel: number): DungeonOption[] {
  return dungeons.filter((d) => d.level != null && !dungeonLevels(d.level, from, to, maxLevel));
}

const signed = (n: number) => (n > 0 ? `+${n}` : String(n));

type DungeonState = "" | "forced" | "quests" | "excluded";

/** The dungeons by level, a click cycling one: automatic → forced → forced with its quests →
 * excluded. Above them, the levels they are run at, which the guide keeps to. */
export function DungeonPanel({ dungeons, choices, onChange, from, to, maxLevel, onLevels }: Props) {
  const t = useT();
  const areaName = useAreaName();
  const { forced, forcedQuests, excluded, levels: chosen } = choices;
  const stateOf = (id: number): DungeonState =>
    forcedQuests.includes(id) ? "quests" : forced.includes(id) ? "forced" : excluded.includes(id) ? "excluded" : "";
  const NEXT: Record<DungeonState, DungeonState> = { "": "forced", forced: "quests", quests: "excluded", excluded: "" };
  const cycle = (id: number) => put(id, NEXT[stateOf(id)]);
  /** Puts dungeon `id` in state `next` ("": automatic), out of the others. */
  const put = (id: number, next: DungeonState) => {
    const without = (ids: number[]) => ids.filter((x) => x !== id);
    onChange({
      forced: next === "forced" ? [...without(forced), id] : without(forced),
      forcedQuests: next === "quests" ? [...without(forcedQuests), id] : without(forcedQuests),
      excluded: next === "excluded" ? [...without(excluded), id] : without(excluded),
      // A level of its own is kept for a forced dungeon only.
      levels: next === "forced" || next === "quests" ? chosen : chosen.filter(([area]) => area !== id),
    });
  };
  const LABELS: Record<DungeonState, string> = { "": t("Automatique"), forced: t("Forcé"), quests: t("Forcé avec ses quêtes"), excluded: t("Exclu") };
  const ICONS: Record<DungeonState, string> = { "": "", forced: "★", quests: "★★", excluded: "✕" };
  const GROUPS: { state: Exclude<DungeonState, "">; ids: number[] }[] = [
    { state: "forced", ids: forced },
    { state: "quests", ids: forcedQuests },
    { state: "excluded", ids: excluded },
  ];
  const byId = new Map(dungeons.map((d) => [d.id, d]));
  const ownLevel = (id: number) => chosen.find(([area]) => area === id)?.[1];
  /** The player's own level for a forced dungeon (null: the levels around its monsters'). */
  const setOwn = (id: number, level: number | null) =>
    onChange({ ...choices, levels: [...chosen.filter(([area]) => area !== id), ...(level == null ? [] : [[id, level] as [number, number]])] });
  const range = (d: DungeonOption) => (d.level == null ? null : dungeonLevels(d.level, from, to, maxLevel));
  const levels = (d: DungeonOption) => {
    const own = ownLevel(d.id);
    if (own != null) return String(own);
    const r = range(d);
    return r ? (r[0] === r[1] ? String(r[0]) : `${r[0]}–${r[1]}`) : "";
  };
  const empty = dungeonsWithoutLevels(dungeons, from, to, maxLevel);
  // A slider with two handles: the lowest and the highest level, around the dungeon's.
  const [low, high] = [-10, 10];
  const at = (n: number) => String((n - low) / (high - low));

  return (
    <section className="panel dungeons-layout">
      <div>
        <h3>
          {t("Donjons")}
          <Hint text={`${t("Cliquez un donjon : forcé → forcé avec ses quêtes → exclu → automatique.")} ${t("Forcé : fait même s'il n'est pas rentable.")} ${t("Avec ses quêtes : toutes celles que le personnage peut prendre, même peu rentables.")} ${t("Un donjon forcé peut se faire à votre niveau : choisissez-le sous son nom.")}`} />
        </h3>
        <div className="dungeon-levels">
          <span>
            {t("Niveau des donjons, par rapport à leurs monstres")}
            <Hint text={t("Le guide fait chaque donjon entre ces niveaux, jamais en dehors : les niveaux affichés sous chaque donjon. Le profil de progression les remet à sa valeur.")} />
          </span>
          <div className="dual-range" style={{ ["--from" as string]: at(from), ["--to" as string]: at(to) }}>
            <input type="range" aria-label={t("Minimum")} min={low} max={high} value={from} onChange={(e) => onLevels(Math.min(Number(e.target.value), to), to)} />
            <input type="range" aria-label={t("Maximum")} min={low} max={high} value={to} onChange={(e) => onLevels(from, Math.max(Number(e.target.value), from))} />
          </div>
          <strong className="dual-range-value">{t("de {from} à {to}", { from: signed(from), to: signed(to) })}</strong>
        </div>
        {empty.length > 0 && (
          <p className="error">{t("Aucun niveau possible pour : {names}. Changez le minimum ou le maximum.", { names: empty.map(areaName).join(", ") })}</p>
        )}
        <div className="dungeon-grid">
          {dungeons.map((d) => {
            const state = stateOf(d.id);
            return (
              <button key={d.id} className={`dungeon ${state}`} onClick={() => cycle(d.id)} title={LABELS[state]} aria-label={`${areaName(d)} ${levels(d)} : ${LABELS[state]}`}>
                <span>
                  {state === "forced" && <b aria-hidden>★ </b>}
                  {state === "quests" && <b aria-hidden>★★ </b>}
                  {state === "excluded" && <b aria-hidden>✕ </b>}
                  {areaName(d)}
                </span>
                <small>{levels(d)}</small>
              </button>
            );
          })}
        </div>
      </div>
      {/* Beside them, the dungeons chosen, by state, with the levels they are run at. */}
      <div className="zone-panel-side">
        {GROUPS.every((g) => g.ids.length === 0) && <p className="muted">{t("Aucun donjon choisi : le guide fait ceux qui valent le coup.")}</p>}
        {GROUPS.filter((g) => g.ids.length > 0).map(({ state, ids }) => (
          <div key={state} className={`zone-group dungeon-group ${state}`}>
            <div className="zone-group-head">
              {ICONS[state]} {LABELS[state]}
              <span className="muted">{ids.length}</span>
            </div>
            <ul className="zone-list">
              {ids.map((id) => {
                const d = byId.get(id);
                return (
                  <li key={id}>
                    <span className="zone-name">
                      {d ? areaName(d) : String(id)}
                      <small>{d ? levels(d) : ""}</small>
                    </span>
                    {d && state !== "excluded" && (
                      <select
                        className="dungeon-level"
                        value={ownLevel(id) ?? ""}
                        aria-label={`${areaName(d)} : ${t("Niveau du donjon")}`}
                        title={t("Niveau du donjon")}
                        onChange={(e) => setOwn(id, e.target.value === "" ? null : Number(e.target.value))}
                      >
                        <option value="">{t("Automatique")}</option>
                        {ownChoices(range(d), maxLevel).map((l) => (
                          <option key={l} value={l}>
                            {t("niveau {n}", { n: l })}
                          </option>
                        ))}
                      </select>
                    )}
                    {(["forced", "quests", "excluded"] as const)
                      .filter((o) => o !== state)
                      .map((o) => (
                        <button key={o} className="icon" title={LABELS[o]} onClick={() => put(id, o)}>
                          {ICONS[o]}
                        </button>
                      ))}
                    <button className="icon" title={t("Retirer")} onClick={() => put(id, "")}>
                      −
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </div>
    </section>
  );
}
