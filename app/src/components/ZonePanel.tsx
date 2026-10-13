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
  excluded: number[];
  onChange: (preferred: number[], excluded: number[]) => void;
}

const CONTINENTS: Record<number, string> = { 0: "Royaumes de l'Est", 1: "Kalimdor", 530: "Outreterre" };

/** Beside the map: one search to prefer or exclude a zone, and the zones chosen, by state. */
export function ZonePanel({ zones, preferred, excluded, onChange }: Props) {
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
  const without = (ids: number[], id: number) => ids.filter((x) => x !== id);
  const prefer = (id: number) => onChange([...without(preferred, id), id], without(excluded, id));
  const exclude = (id: number) => onChange(without(preferred, id), [...without(excluded, id), id]);
  const remove = (id: number) => onChange(without(preferred, id), without(excluded, id));

  const group = (state: "preferred" | "excluded", ids: number[]) =>
    ids.length > 0 && (
      <div className={`zone-group ${state}`}>
        <div className="zone-group-head">
          {state === "preferred" ? t("★ Privilégiées") : t("✕ Exclues")}
          <span className="muted">{ids.length}</span>
        </div>
        <ul className="zone-list">
          {ids.map((id) => (
            <li key={id}>
              <span className="zone-name">
                {name(id)}
                <small>{continent(byId.get(id))}</small>
              </span>
              <button className="icon" title={state === "preferred" ? t("Exclure") : t("Privilégier")} onClick={() => (state === "preferred" ? exclude(id) : prefer(id))}>
                {state === "preferred" ? "✕" : "★"}
              </button>
              <button className="icon" title={t("Retirer")} onClick={() => remove(id)}>
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
            <li key={z.id} className={preferred.includes(z.id) ? "preferred" : excluded.includes(z.id) ? "excluded" : ""}>
              <span className="zone-name">
                {areaName(z)}
                <small>{continent(z)}</small>
              </span>
              <button className="icon prefer" title={t("Privilégier")} aria-pressed={preferred.includes(z.id)} onClick={() => (preferred.includes(z.id) ? remove(z.id) : prefer(z.id))}>
                ★
              </button>
              <button className="icon exclude" title={t("Exclure")} aria-pressed={excluded.includes(z.id)} onClick={() => (excluded.includes(z.id) ? remove(z.id) : exclude(z.id))}>
                ✕
              </button>
            </li>
          ))}
          {matches.length === 0 && <li className="muted">{t("Aucune zone trouvée.")}</li>}
        </ul>
      )}
      {group("preferred", preferred)}
      {group("excluded", excluded)}
    </div>
  );
}
