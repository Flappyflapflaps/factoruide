import { useMemo, useState } from "react";
import { useAreaName, useT } from "../i18n";

interface Zone {
  id: number;
  name: string;
  names?: Record<string, string>;
  continent: number;
}

interface Props {
  zones: Zone[];
  preferred: number[];
  forced: number[];
  excluded: number[];
  onChange: (preferred: number[], forced: number[], excluded: number[]) => void;
}

type State = "preferred" | "forced" | "excluded";

const CONTINENTS: Record<number, string> = { 0: "Royaumes de l'Est", 1: "Kalimdor", 530: "Outreterre" };

/** The states a zone can take, with their button: preferred (★), forced (⚑: every quest), excluded (✕). */
const STATES: { state: State; icon: string; action: string; group: string }[] = [
  { state: "preferred", icon: "★", action: "Privilégier", group: "★ Privilégiées" },
  { state: "forced", icon: "⚑", action: "Forcer (toutes ses quêtes)", group: "⚑ Forcées" },
  { state: "excluded", icon: "✕", action: "Exclure", group: "✕ Exclues" },
];

/** Beside the map: one search to prefer, force or exclude a zone, and the zones chosen, by state. */
export function ZonePanel({ zones, preferred, forced, excluded, onChange }: Props) {
  const t = useT();
  const areaName = useAreaName();
  const [query, setQuery] = useState("");
  const byId = useMemo(() => new Map(zones.map((z) => [z.id, z])), [zones]);
  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    return zones.filter((z) => z.name.toLowerCase().includes(q) || areaName(z).toLowerCase().includes(q)).slice(0, 8);
  }, [query, zones, areaName]);
  const continent = (z: Zone | undefined) => (z && CONTINENTS[z.continent] ? t(CONTINENTS[z.continent]) : "");
  const name = (id: number) => (byId.has(id) ? areaName(byId.get(id)!) : String(id));
  const lists: Record<State, number[]> = { preferred, forced, excluded };
  const stateOf = (id: number) => STATES.find((s) => lists[s.state].includes(id))?.state;
  // Puts a zone in one state (none: neutral), out of the others.
  const put = (id: number, state: State | undefined) => {
    const next = (s: State) => [...lists[s].filter((x) => x !== id), ...(s === state ? [id] : [])];
    onChange(next("preferred"), next("forced"), next("excluded"));
  };
  const toggle = (id: number, state: State) => put(id, stateOf(id) === state ? undefined : state);

  const group = ({ state, group: title }: (typeof STATES)[number]) =>
    lists[state].length > 0 && (
      <div key={state} className={`zone-group ${state}`}>
        <div className="zone-group-head">
          {t(title)}
          <span className="muted">{lists[state].length}</span>
        </div>
        <ul className="zone-list">
          {lists[state].map((id) => (
            <li key={id}>
              <span className="zone-name">
                {name(id)}
                <small>{continent(byId.get(id))}</small>
              </span>
              {STATES.filter((o) => o.state !== state).map((o) => (
                <button key={o.state} className="icon" title={t(o.action)} onClick={() => put(id, o.state)}>
                  {o.icon}
                </button>
              ))}
              <button className="icon" title={t("Retirer")} onClick={() => put(id, undefined)}>
                −
              </button>
            </li>
          ))}
        </ul>
      </div>
    );

  return (
    <div className="zone-panel-side">
      <input type="search" placeholder={t("Rechercher une zone…")} value={query} onChange={(e) => setQuery(e.target.value)} />
      {query.trim() && (
        <ul className="zone-list results">
          {matches.map((z) => (
            <li key={z.id} className={stateOf(z.id) ?? ""}>
              <span className="zone-name">
                {areaName(z)}
                <small>{continent(z)}</small>
              </span>
              {STATES.map((o) => (
                <button
                  key={o.state}
                  className={`icon ${o.state === "preferred" ? "prefer" : o.state === "forced" ? "force" : "exclude"}`}
                  title={t(o.action)}
                  aria-pressed={stateOf(z.id) === o.state}
                  onClick={() => toggle(z.id, o.state)}
                >
                  {o.icon}
                </button>
              ))}
            </li>
          ))}
          {matches.length === 0 && <li className="muted">{t("Aucune zone trouvée.")}</li>}
        </ul>
      )}
      {STATES.map(group)}
    </div>
  );
}
