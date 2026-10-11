import { Fragment, useMemo, useState, type ReactNode } from "react";
import { fmtTime, type Route, type Step } from "../api";
import { BreakdownBars } from "./BreakdownBars";
import { LevelChart } from "./LevelChart";
import { useT } from "../i18n";
import type { WorldPoint } from "./WorldMap";

const KIND_LABELS: Record<string, string> = {
  accept: "Prendre",
  objective: "Faire",
  turnin: "Rendre",
  fly: "Vol",
  flight_master: "Point de vol",
  link: "Transport",
  grind: "Farmer",
  train: "Entraîneur",
  hearth: "Pierre de foyer",
  bind: "Auberge",
  dungeon: "Donjon",
  profession: "Métier",
  practice: "Pratique",
  deathskip: "Mort volontaire",
  abandon: "Abandonner",
};

/** Why the planner left quests out (`fg_route` model), English as sent, French key of the catalog. */
const SKIP_REASONS: Record<string, string> = {
  "elite objective": "objectif élite",
  "excluded by configuration": "exclue par la configuration",
  "exploration area unknown": "zone d'exploration inconnue",
  "holiday/profession/event category": "catégorie fête, métier ou événement",
  "needs a profession, reputation or spell": "demande un métier, une réputation ou un sort",
  "no reachable quest giver": "aucun donneur accessible",
  "no reachable turn-in": "aucun PNJ de rendu accessible",
  "objective item from elite mobs": "objet d'objectif sur des élites",
  "objective item source unknown": "source de l'objet d'objectif inconnue",
  "objectives and turn-in unknown (catalogue only)": "objectifs et rendu inconnus (catalogue seulement)",
  "objectives unknown (only described)": "objectifs inconnus (seulement décrits)",
  "PvP (battleground) quest": "quête JcJ (champ de bataille)",
  repeatable: "répétable",
  "reputation/spell objective": "objectif de réputation ou de sort",
  "started by a rare drop": "commence par un objet rare",
};

interface Segment {
  /** Zone, or dungeon name; null: steps done before any place (abandoning quests). */
  zone: string | null;
  dungeon: boolean;
  start: number;
  end: number;
  fromLevel: number;
  toLevel: number;
  /** Indices of the first and last steps. */
  first: number;
  last: number;
}

const steps = (s: Segment) => s.last - s.first + 1;

/** Consecutive steps in the same zone; a dungeon run is a segment of its own. */
function segments(route: Route): Segment[] {
  const out: Segment[] = [];
  let previousTime = 0;
  route.steps.forEach((s, i) => {
    const last = out[out.length - 1];
    const dungeon = s.kind === "dungeon";
    // Steps done anywhere (practicing a profession, abandoning a quest) have no place: they
    // belong to the zone the character is in.
    const zone = dungeon ? (s.target?.name ?? s.zone ?? null) : (s.zone ?? (last && !last.dungeon ? last.zone : null));
    if (last && !dungeon && !last.dungeon && last.zone === zone) {
      last.end = s.time;
      last.toLevel = s.level;
      last.last = i;
    } else {
      out.push({ zone, dungeon, start: previousTime, end: s.time, fromLevel: s.level, toLevel: s.level, first: i, last: i });
    }
    previousTime = s.time;
  });
  // Fold short passages (a couple of steps, a few minutes) into the previous segment.
  const merged: Segment[] = [];
  for (const s of out) {
    const last = merged[merged.length - 1];
    const short = steps(s) <= 2 && s.end - s.start < 300;
    if (last && !last.dungeon && !s.dungeon && (short || last.zone === s.zone)) {
      last.end = s.end;
      last.toLevel = s.toLevel;
      last.last = s.last;
    } else {
      merged.push({ ...s });
    }
  }
  return merged;
}

/** "N plannable quests, start …" then "skipped N: reason" lines (`fg_route::job::notes`). */
function Notes({ notes }: { notes: string[] }) {
  const t = useT();
  const skipped = notes.flatMap((n) => {
    const m = /^skipped (\d+): (.+)$/.exec(n);
    return m ? [{ n: Number(m[1]), reason: SKIP_REASONS[m[2]] ? t(SKIP_REASONS[m[2]]) : m[2] }] : [];
  });
  const others = notes.filter((n) => !/^skipped \d+: /.test(n));
  const plannable = others.map((n) => /^(\d+) plannable quests, start (.+)$/.exec(n)).find(Boolean);
  const where = plannable && /^where the character stands \((.+)\)$/.exec(plannable[2]);
  const total = skipped.reduce((s, r) => s + r.n, 0);
  return (
    <details className="notes">
      <summary>
        {plannable
          ? t("{n} quêtes possibles, {skipped} écartées", { n: plannable[1], skipped: total })
          : t("Notes du calcul ({n})", { n: notes.length })}
      </summary>
      {plannable && (
        <p className="muted">{where ? t("Départ : là où se trouve le personnage ({place})", { place: where[1] }) : t("Départ : {place}", { place: plannable[2].replace(/^at /, "") })}</p>
      )}
      {others
        .filter((n) => !/^\d+ plannable quests, /.test(n))
        .map((n, i) => (
          <p key={i} className="muted">
            {n}
          </p>
        ))}
      {skipped.length > 0 && (
        <table className="table compact notes-table">
          <tbody>
            {[...skipped]
              .sort((a, b) => b.n - a.n)
              .map((r) => (
                <tr key={r.reason}>
                  <td className="num">{r.n}</td>
                  <td>{r.reason}</td>
                </tr>
              ))}
          </tbody>
        </table>
      )}
    </details>
  );
}

/** Where the XP comes from, as one stacked bar and its legend. */
function XpTile({ route }: { route: Route }) {
  const t = useT();
  const b = route.breakdown;
  const parts = [
    { label: t("Quêtes"), value: b.quest_xp, color: "var(--cat-1)" },
    { label: t("Mobs"), value: b.mob_xp, color: "var(--cat-3)" },
    { label: t("Farm en chemin"), value: b.farm_xp ?? 0, color: "var(--cat-4)" },
    { label: t("Farm"), value: b.grind_xp, color: "var(--cat-2)" },
    { label: t("Exploration"), value: b.explore_xp ?? 0, color: "var(--cat-7)" },
  ].filter((p) => p.value > 0);
  const total = parts.reduce((s, p) => s + p.value, 0);
  const pct = (v: number) => Math.round((v / Math.max(1, total)) * 100);
  return (
    <div className="tile">
      <span>XP</span>
      <strong>{Math.round(total / 1000)}k</strong>
      <div className="stack" role="img" aria-label={parts.map((p) => `${p.label} ${pct(p.value)} %`).join(", ")}>
        {parts.map((p) => (
          <span key={p.label} style={{ flexGrow: p.value, background: p.color }} title={`${p.label} : ${Math.round(p.value / 1000)}k (${pct(p.value)} %)`} />
        ))}
      </div>
      <small className="stack-legend">
        {parts
          .filter((p) => pct(p.value) >= 1)
          .map((p) => (
          <span key={p.label}>
            <i className="swatch" style={{ background: p.color }} /> {p.label} {pct(p.value)} %
          </span>
        ))}
      </small>
    </div>
  );
}

interface Props {
  route: Route;
  notes?: string[];
  /** Shown under the play time (the difference with the previous version). */
  compare?: ReactNode;
  /** Steps of the zone row opened (their positions), null when closed. */
  onHighlight?: (points: WorldPoint[] | null) => void;
}

export function RouteView({ route, notes, compare, onHighlight }: Props) {
  const t = useT();
  const [filter, setFilter] = useState("");
  const [tab, setTab] = useState<"zones" | "steps">("zones");
  const [open, setOpen] = useState<number | null>(null);
  const b = route.breakdown;
  const segs = useMemo(() => segments(route), [route]);
  const kindLabel = (s: Step) => t(KIND_LABELS[s.kind] ?? s.kind);
  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase();
    let zone: string | undefined;
    return route.steps
      .map((s, i) => {
        zone = s.zone ?? zone;
        return { ...s, zone, n: i + 1, start: i > 0 ? route.steps[i - 1].time : 0 };
      })
      .filter((s) => !q || s.text.toLowerCase().includes(q) || (s.zone ?? "").toLowerCase().includes(q) || kindLabel(s).toLowerCase().includes(q));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route, filter, t]);
  const flights = route.steps.filter((s) => s.kind === "fly").length;

  const toggle = (i: number) => {
    const next = open === i ? null : i;
    setOpen(next);
    const s = segs[i];
    onHighlight?.(next === null ? null : route.steps.slice(s.first, s.last + 1).flatMap((x) => (x.world ? [x.world as WorldPoint] : [])));
  };
  const duration = (seconds: number) => (seconds >= 30 ? fmtTime(seconds) : "");

  return (
    <div className="route-view">
      <div className="tiles">
        <div className="tile tile-hero">
          <span>{t("Temps de jeu estimé")}</span>
          <strong>{route.total_time_text}</strong>
          <small>
            {t("niveau {from} → {to}", { from: route.from_level, to: route.to_level })}
            {compare && <> · {compare}</>}
          </small>
        </div>
        <div className="tile">
          <span>{t("Quêtes")}</span>
          <strong>{route.quests}</strong>
          <small>{t("{n} étapes", { n: route.steps.length })}</small>
        </div>
        <div className="tile">
          <span>{t("Donjons")}</span>
          <strong>{b.dungeon_runs}</strong>
          <small>{b.dungeon_runs > 0 ? fmtTime(b.dungeons) : "—"}</small>
        </div>
        <div className="tile">
          <span>{t("Pierres de foyer")}</span>
          <strong>{b.hearths}</strong>
          <small>{t("{n} vols", { n: flights })}</small>
        </div>
        <XpTile route={route} />
      </div>
      {b.missing_class_quests > 0 && (
        <div className="notice warn">{t("Quêtes de classe obligatoires manquées : {n}", { n: b.missing_class_quests })}</div>
      )}

      <div className="charts">
        <LevelChart steps={route.steps} fromLevel={route.from_level} toLevel={route.to_level} />
        <BreakdownBars breakdown={b} />
      </div>

      {notes && notes.length > 0 && <Notes notes={notes} />}

      <div className="view-tabs" role="tablist">
        <button role="tab" aria-selected={tab === "zones"} className={tab === "zones" ? "active" : ""} onClick={() => setTab("zones")}>
          {t("Parcours par zone")}
        </button>
        <button role="tab" aria-selected={tab === "steps"} className={tab === "steps" ? "active" : ""} onClick={() => setTab("steps")}>
          {t("Toutes les étapes")}
        </button>
      </div>

      {tab === "zones" ? (
        <table className="table route-zones">
          <thead>
            <tr>
              <th className="num">{t("Début")}</th>
              <th className="num">{t("Durée")}</th>
              <th>{t("Niveaux")}</th>
              <th>{t("Zone")}</th>
              <th className="num">{t("Étapes")}</th>
            </tr>
          </thead>
          <tbody>
            {segs.map((s, i) => (
              <Fragment key={i}>
                <tr className={`clickable${open === i ? " selected" : ""}`} onClick={() => toggle(i)} aria-expanded={open === i}>
                  <td className="num">{fmtTime(s.start)}</td>
                  <td className="num">{duration(s.end - s.start)}</td>
                  <td>{s.fromLevel === s.toLevel ? s.fromLevel : `${s.fromLevel} → ${s.toLevel}`}</td>
                  <td>
                    <span className="disclosure" aria-hidden>
                      {open === i ? "▾" : "▸"}
                    </span>{" "}
                    {s.dungeon && <span className="kind kind-dungeon">{t("Donjon")}</span>} {s.zone ?? <em className="muted">{t("Préparation")}</em>}
                  </td>
                  <td className="num">{steps(s)}</td>
                </tr>
                {open === i &&
                  route.steps.slice(s.first, s.last + 1).map((x, j) => (
                    <tr key={`${i}-${j}`} className="sub-row">
                      <td className="num">{fmtTime(x.time)}</td>
                      <td />
                      <td>{x.level}</td>
                      <td colSpan={2}>
                        <span className={`kind kind-${x.kind}`}>{kindLabel(x)}</span> {x.text}
                        {x.bg && <span className="along"> {t("en chemin")}</span>}
                      </td>
                    </tr>
                  ))}
              </Fragment>
            ))}
          </tbody>
        </table>
      ) : (
        <>
          <input className="search" placeholder={t("Filtrer (quête, zone, type…)")} value={filter} onChange={(e) => setFilter(e.target.value)} />
          <table className="table route-steps">
            <thead>
              <tr>
                <th className="num">#</th>
                <th className="num">{t("Début")}</th>
                <th className="num">{t("Durée")}</th>
                <th className="num">{t("Niv.")}</th>
                <th>{t("Type")}</th>
                <th>{t("Étape")}</th>
                <th>{t("Coords")}</th>
              </tr>
            </thead>
            <tbody>
              {shown.slice(0, 2000).map((s, i, all) => (
                <Fragment key={s.n}>
                  {(i === 0 || all[i - 1].zone !== s.zone) && (
                    <tr className="zone-row">
                      <td colSpan={7}>{s.zone ?? t("Préparation")}</td>
                    </tr>
                  )}
                  <tr>
                    <td className="num muted">{s.n}</td>
                    <td className="num">{fmtTime(s.start)}</td>
                    <td className={`num${s.time - s.start >= 1800 ? " long" : ""}`}>{duration(s.time - s.start)}</td>
                    <td className="num">{s.level}</td>
                    <td>
                      <span className={`kind kind-${s.kind}`}>{kindLabel(s)}</span>
                    </td>
                    <td>
                      {s.text}
                      {s.bg && <span className="along"> {t("en chemin")}</span>}
                    </td>
                    <td className="coords">{s.map ? `${s.map.x.toFixed(1)}, ${s.map.y.toFixed(1)}` : ""}</td>
                  </tr>
                </Fragment>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}
