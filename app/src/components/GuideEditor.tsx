import { Fragment, useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { api, fmtTime, type EditCatalog, type EditOpened, type EditReport, type Farmed, type GuideVersion, type RouteStep, type StepReport, type VersionMeta } from "../api";
import { gameLocale, useLang, useT } from "../i18n";
import { NumberInput } from "./NumberInput";
import { Delta } from "./ResultView";
import { keepTableInView, keepZoneInView, segments, steps as steps_, useFollowPointer, type Segment } from "./RouteView";
import { useWidth } from "./useWidth";
import { ZoneMap, type NumberedStep } from "./ZoneMap";

interface Props {
  /** The version edited: the edited guide is saved as a new version of its guide. */
  version: GuideVersion;
  onClose: () => void;
  onSaved: (meta: VersionMeta) => void;
}

/** Why a step is not possible or does nothing (`fg_route` editor, English as sent), French key of the catalog. */
const REASONS: Record<string, string> = {
  "quest not available": "quête indisponible pour ce personnage",
  "unknown flight path": "point de vol inconnu",
  "unknown inn": "auberge inconnue",
  "unknown dungeon": "donjon inconnu",
  "unknown objective": "objectif inconnu",
  "already turned in": "déjà rendue",
  "already in the log": "déjà dans le journal de quêtes",
  "quest log full": "journal de quêtes plein",
  "prerequisite not done": "prérequis pas fait",
  "exclusive quest taken": "une quête exclusive est déjà prise",
  "gray quest leading nowhere": "quête grise sans suite",
  "dungeon quest (dungeons off)": "quête de donjon (donjons désactivés)",
  "quest not accepted": "quête pas prise",
  "objective done in a dungeon run": "objectif fait pendant un donjon",
  "objectives not done": "objectifs pas faits",
  "flight path already known": "point de vol déjà connu",
  "hearthstone already bound there": "pierre de foyer déjà liée ici",
  "no such spell": "sort pas appris",
  "dungeon already run or left out": "donjon déjà fait ou exclu",
  "not possible": "impossible",
  unreachable: "inaccessible",
  "done before": "déjà fait plus tôt (sur place ou en chemin)",
  "not ready": "pierre de foyer pas encore prête",
  "target level reached": "niveau cible déjà atteint",
  "no mob to kill": "aucun mob à tuer",
  "same flight path": "même point de vol au départ et à l'arrivée",
  "flight path not known": "point de vol de départ pas encore connu",
  "destination not known": "point de vol d'arrivée pas encore connu",
  "no flight between them": "aucun vol entre ces points",
  "no such crossing": "aucune traversée entre ces deux transports",
  "unknown crossing": "transport inconnu",
  "teleport not learned": "téléportation pas encore apprise",
  "unknown trainer": "entraîneur inconnu",
  "nothing to learn": "rien de nouveau à apprendre",
  "level reached": "niveau déjà atteint",
  "quest not in the log": "quête pas dans le journal",
  "quest time limit passed": "temps limite de la quête dépassé",
};

/** Step type of the guide (its color, see .kind-*) and label of each step kind. */
const KINDS: Record<RouteStep["kind"], { css: string; label: string }> = {
  accept: { css: "accept", label: "Prendre" },
  objective: { css: "objective", label: "Faire" },
  turn_in: { css: "turnin", label: "Rendre" },
  flight_master: { css: "flight_master", label: "Point de vol" },
  bind: { css: "bind", label: "Auberge" },
  hearth: { css: "hearth", label: "Pierre de foyer" },
  fly: { css: "fly", label: "Vol" },
  dungeon: { css: "dungeon", label: "Donjon" },
  link: { css: "link", label: "Transport" },
  teleport: { css: "teleport", label: "Téléportation" },
  train: { css: "train", label: "Entraîneur" },
  abandon: { css: "abandon", label: "Abandonner" },
  grind: { css: "grind", label: "Farmer" },
  grind_to: { css: "grind", label: "Farmer" },
};

type AddKind = "accept" | "objective" | "turn_in" | "bind" | "hearth" | "fly" | "flight_master" | "train" | "abandon" | "grind" | "dungeon";

const ADD_KINDS: { kind: AddKind; label: string }[] = [
  { kind: "accept", label: "Prendre une quête" },
  { kind: "objective", label: "Faire une quête" },
  { kind: "turn_in", label: "Rendre une quête" },
  { kind: "bind", label: "Lier la pierre de foyer" },
  { kind: "hearth", label: "Pierre de foyer" },
  { kind: "fly", label: "Prendre un vol" },
  { kind: "flight_master", label: "Apprendre un point de vol" },
  { kind: "train", label: "S'entraîner" },
  { kind: "abandon", label: "Abandonner une quête" },
  { kind: "grind", label: "Farmer" },
  { kind: "dungeon", label: "Donjon" },
];

/** Wait after a change before replaying the guide (several quick changes: one replay). */
const DEBOUNCE_MS = 250;
/** Quests listed at once in the add panel (search to find the others). */
const LIST_MAX = 80;

const sameStep = (a: RouteStep, b: RouteStep) => JSON.stringify(a) === JSON.stringify(b);

/** A step of the draft, with a key kept while it moves (its replay is found by it). */
interface Item {
  key: number;
  step: RouteStep;
}

let nextKey = 0;
const item = (step: RouteStep): Item => ({ key: nextKey++, step });

/**
 * The guide editor: the steps of a guide's route moved, removed and added as a draft, the guide
 * replayed after each change (its time, the steps that are not possible), saved as a new
 * version only when every step is possible.
 */
export function GuideEditor({ version, onClose, onSaved }: Props) {
  const t = useT();
  const locale = gameLocale(useLang());
  const [opened, setOpened] = useState<EditOpened | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [items, setItems] = useState<Item[]>([]);
  // Earlier drafts, for undo.
  const [history, setHistory] = useState<Item[][]>([]);
  // The last replay and the draft it is of: shown (dimmed) until the current draft's arrives.
  const [replayed, setReplayed] = useState<{ items: Item[]; report: EditReport } | null>(null);
  const [saving, setSaving] = useState(false);
  const [filter, setFilter] = useState("");
  // Insert position of the add panel (after this step, -1: first), null when closed.
  const [adding, setAdding] = useState<number | null>(null);
  // Step being moved (cut), placed after the step chosen next.
  const [moving, setMoving] = useState<number | null>(null);
  const [drag, setDrag] = useState<{ from: number; over: number | null } | null>(null);
  const closed = useRef(false);
  // Segments opened (the keys of their steps: they stay open as steps move), the one shown on
  // the map, and the step pointed at (number).
  const [open, setOpen] = useState<Set<number>>(new Set());
  const [focus, setFocus] = useState<number | null>(null);
  const [active, setActive] = useState<number | null>(null);
  const [frame, width] = useWidth(1000);
  const table = useRef<HTMLTableElement>(null);
  const wide = width >= 1150;
  // Height of the actions bar stuck at the top: zone lines and the map stick under it.
  const [bar, setBar] = useState(0);
  const barObserver = useRef<ResizeObserver | null>(null);
  const barRef = useCallback((el: HTMLElement | null) => {
    barObserver.current?.disconnect();
    if (!el) return;
    const update = () => setBar(el.offsetHeight);
    update();
    barObserver.current = new ResizeObserver(update);
    barObserver.current.observe(el);
  }, []);
  const steps = useMemo(() => items.map((x) => x.step), [items]);

  useEffect(() => {
    api
      .editOpen(version.meta.guide, version.meta.version, locale)
      .then((o) => {
        const first = o.steps.map(item);
        setOpened(o);
        setItems(first);
        setReplayed({ items: first, report: o.report });
      })
      .catch((e) => setError(String(e)));
    return () => {
      if (!closed.current) api.editClose().catch(() => {});
    };
    // Opened once per version (the language is kept from the opening).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [version.meta.guide, version.meta.version]);

  // Replay the draft after each change.
  useEffect(() => {
    if (!opened || replayed?.items === items) return;
    let stale = false;
    const timer = setTimeout(() => {
      api
        .editSimulate(steps, locale)
        .then((report) => {
          if (stale) return;
          setReplayed({ items, report });
          setError(null);
        })
        .catch((e) => {
          if (!stale) setError(String(e));
        });
    }, DEBOUNCE_MS);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [items, steps, opened, replayed, locale]);

  const change = (next: Item[]) => {
    setHistory((h) => [...h.slice(-99), items]);
    setItems(next);
    setMoving(null);
  };
  const undo = () => {
    const previous = history[history.length - 1];
    if (!previous) return;
    setHistory(history.slice(0, -1));
    setItems(previous);
    setMoving(null);
  };
  /** Step `from` placed after step `after` (-1: first). */
  const move = (from: number, after: number) => {
    if (from === after || from === after + 1) return;
    const next = [...items];
    const [moved] = next.splice(from, 1);
    next.splice(after < from ? after + 1 : after, 0, moved);
    change(next);
  };
  const remove = (i: number) => change(items.filter((_, k) => k !== i));
  const replace = (i: number, step: RouteStep) => change(items.map((x, k) => (k === i ? { ...x, step } : x)));
  const insert = (after: number, step: RouteStep) => {
    const next = [...items];
    next.splice(after + 1, 0, item(step));
    change(next);
    setAdding(after + 1);
  };

  // The replay of each step, by key (the last replay while the current one runs).
  const reports = useMemo(() => new Map(replayed?.items.map((x, i) => [x.key, replayed.report.steps[i]])), [replayed]);
  const baseline = opened?.report;
  const current = replayed?.items === items ? replayed.report : null;
  const shown = current ?? replayed?.report ?? null;
  const changed = history.length > 0 && !(steps.length === opened?.steps.length && steps.every((s, i) => sameStep(s, opened.steps[i])));
  const catalog = opened?.catalog;
  const names = useNames(catalog);
  // Consecutive steps in the same zone (where each step's line is).
  const segs = useMemo(() => {
    let last = { time: 0, level: version.outcome.route.from_level };
    return segments(
      items.map((x) => {
        const r = reports.get(x.key);
        const line = r?.line;
        if (r && r.status !== "error") last = { time: r.end, level: r.level };
        const dungeon = x.step.kind === "dungeon" ? names.dungeons.get(x.step.dungeon)?.name : undefined;
        return { kind: x.step.kind, zone: line?.zone, target: dungeon ? { name: dungeon } : undefined, ...last };
      }),
    );
  }, [items, reports, names, version]);
  const segOf = useMemo(() => {
    const out: number[] = [];
    segs.forEach((s, k) => {
      for (let i = s.first; i <= s.last; i++) out[i] = k;
    });
    return out;
  }, [segs]);
  // Beside the table, the map shows the zone under the pointer, else the first opened one in view.
  useFollowPointer(table, wide, "tr.edit-row", setFocus, [open, filter, items]);

  if (error && !opened) {
    return (
      <div>
        <button className="link" onClick={onClose}>
          {t("← Retour au guide")}
        </button>
        <div className="error">{error}</div>
      </div>
    );
  }
  if (!opened || !catalog || !baseline || !shown) return <div className="empty">{t("Ouverture du guide dans l'éditeur…")}</div>;

  const label = (s: RouteStep) => names.label(s, t);
  const q = filter.trim().toLowerCase();
  const matches = (x: Item) => {
    if (!q) return true;
    const r = reports.get(x.key);
    return (r?.line?.text ?? label(x.step)).toLowerCase().includes(q) || (r?.line?.zone ?? "").toLowerCase().includes(q);
  };
  const isOpen = (s: Segment) => !!q || items.slice(s.first, s.last + 1).some((x) => open.has(x.key));
  const toggle = (k: number) => {
    const s = segs[k];
    const keys = items.slice(s.first, s.last + 1).map((x) => x.key);
    const next = new Set(open);
    if (isOpen(s)) {
      for (const key of keys) next.delete(key);
    } else {
      for (const key of keys) next.add(key);
      setFocus(k);
    }
    setOpen(next);
  };
  /** Opens the segment of step `i` and scrolls to it. */
  const reveal = (i: number) => {
    const k = segOf[i];
    if (k != null && !isOpen(segs[k])) toggle(k);
    setTimeout(() => document.getElementById(`edit-step-${i}`)?.scrollIntoView({ block: "center" }), 50);
  };

  // Quests left unfinished by the edit (the original guide's do not count: taken before the
  // target level, turned in after it).
  const before = new Set(baseline.unfinished.map((u) => u.id));
  const unfinished = shown.unfinished.filter((u) => !before.has(u.id));
  const shortfall = shown.final_grind - baseline.final_grind;
  const firstError = current?.steps.findIndex((r) => r.status === "error") ?? -1;
  const canSave = !!current && current.errors === 0 && changed && !saving;
  const duration = (seconds: number) => (seconds >= 30 ? fmtTime(seconds) : "");

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const meta = await api.editSave(steps);
      closed.current = true;
      onSaved(meta);
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  };
  const leave = () => {
    if (changed && !confirm(t("Quitter l'édition sans enregistrer ? Les modifications seront perdues."))) return;
    closed.current = true;
    api.editClose().catch(() => {});
    onClose();
  };
  const addPanel = (at: number) => (
    <tr className="add-row">
      <td colSpan={COLUMNS}>
        <AddPanel at={at} level={(at >= 0 ? reports.get(items[at]?.key)?.level : undefined) || version.outcome.route.from_level} steps={steps} catalog={catalog} names={names} onAdd={(s) => insert(at, s)} onClose={() => setAdding(null)} />
      </td>
    </tr>
  );
  // The focused segment's steps that have a place, on its zone map.
  const focused = focus != null ? segs[focus] : null;
  const mapSteps: NumberedStep[] = focused
    ? items.slice(focused.first, focused.last + 1).flatMap((x, k) => {
        const r = reports.get(x.key);
        const l = r?.line;
        return r && l?.map ? [{ kind: l.kind, text: l.text, map: l.map, level: r.level, time: r.end, n: focused.first + k + 1 }] : [];
      })
    : [];

  return (
    <div className="guide-editor" style={{ "--sticky-top": `${bar}px` } as CSSProperties}>
      <header className="result-head">
        <div>
          <h2>
            {version.outcome.route.name} <span className="version-tag">v{version.meta.version}</span> <span className="tag draft">{t("brouillon")}</span>
          </h2>
          <div className="muted">{t("Les modifications ne sont pas enregistrées : le guide est rejoué après chacune, « Enregistrer » en fait une nouvelle version.")}</div>
        </div>
      </header>
      <div className="actions editor-actions" ref={barRef}>
        <button className="primary" disabled={!canSave} onClick={save} title={current && current.errors > 0 ? t("Corrigez les étapes impossibles pour enregistrer") : undefined}>
          {saving ? t("Enregistrement…") : t("Enregistrer (nouvelle version)")}
        </button>
        <button onClick={undo} disabled={history.length === 0}>
          {t("Annuler la dernière modification")}
        </button>
        <button onClick={() => setAdding(adding === -1 ? null : -1)}>{t("Ajouter une étape au début")}</button>
        <button onClick={leave}>{t("Quitter sans enregistrer")}</button>
      </div>

      <div className="tiles">
        <div className="tile tile-hero">
          <span>{t("Temps de jeu estimé")}</span>
          <strong className={current ? "" : "pending"}>{shown.total_time_text}</strong>
          <small>
            <Delta seconds={shown.total_time - baseline.total_time} /> {t("vs version {n}", { n: version.meta.version })}
          </small>
        </div>
        <div className="tile">
          <span>{t("Étapes")}</span>
          <strong>{steps.length}</strong>
          <small>{current ? t("simulé") : t("simulation…")}</small>
        </div>
        <div className="tile">
          <span>{t("Farm final")}</span>
          <strong>{shown.final_grind >= 30 ? fmtTime(shown.final_grind) : "—"}</strong>
          <small>{t("jusqu'au niveau {level}", { level: version.outcome.route.to_level })}</small>
        </div>
      </div>

      {opened.rebuilt && (
        <div className="notice">{t("Ce guide a été calculé avant l'éditeur : ses étapes ont été relues depuis le guide, son temps peut un peu différer.")}</div>
      )}
      {error && <div className="error">{error}</div>}
      {current && current.errors > 0 && (
        <div className="error editor-status">
          {t("{n} étape(s) impossible(s) : le guide ne peut pas être enregistré.", { n: current.errors })}{" "}
          {firstError >= 0 && (
            <button className="link" onClick={() => reveal(firstError)}>
              {t("Voir la première")}
            </button>
          )}
        </div>
      )}
      {(unfinished.length > 0 || shortfall >= 60) && (
        <div className="notice warn">
          <strong>{t("Guide incomplet")}</strong> — {t("il peut tout de même être enregistré.")}
          {unfinished.length > 0 && <div>{t("Quêtes prises jamais rendues : {list}", { list: unfinished.map((u) => u.name).join(", ") })}</div>}
          {shortfall >= 60 && <div>{t("Les étapes n'amènent plus au niveau {level} : {time} de farm ajoutés à la fin.", { level: version.outcome.route.to_level, time: fmtTime(shortfall) })}</div>}
        </div>
      )}
      {current && current.errors === 0 && unfinished.length === 0 && shortfall < 60 && changed && <div className="success">{t("Toutes les étapes sont possibles.")}</div>}

      <div className="route-toolbar">
        <input className="search" placeholder={t("Filtrer (quête, zone, type…)")} value={filter} onChange={(e) => setFilter(e.target.value)} />
        <button onClick={() => setOpen(new Set(items.map((x) => x.key)))} disabled={!!q}>
          {t("Tout déplier")}
        </button>
        <button
          onClick={() => {
            setOpen(new Set());
            setFocus(null);
            keepTableInView(table.current);
          }}
          disabled={!!q}
        >
          {t("Tout replier")}
        </button>
      </div>
      {moving !== null && (
        <div className="notice editor-moving">
          {t("Déplacement de l'étape {n} : choisissez « Placer ici » sur l'étape après laquelle la mettre.", { n: moving + 1 })}{" "}
          <button className="link" onClick={() => setMoving(null)}>
            {t("Annuler")}
          </button>
        </div>
      )}

      <div className={`route-layout${wide ? " wide" : ""}`} ref={frame}>
        <table ref={table} className={`table route-table editor-table${current ? "" : " stale"}`}>
          <colgroup>
            <col className="c-handle" />
            <col className="c-time" />
            <col className="c-time" />
            <col className="c-level" />
            <col className="c-n" />
            <col />
            <col className="c-actions" />
          </colgroup>
          <thead>
            <tr>
              <th />
              <th className="num">{t("Début")}</th>
              <th className="num">{t("Durée")}</th>
              <th>{t("Niv.")}</th>
              <th colSpan={2}>{t("Étape")}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {moving !== null && !q && (
              <tr className="place-row">
                <td colSpan={COLUMNS}>
                  <button className="link" onClick={() => move(moving, -1)}>
                    {t("Placer en premier")}
                  </button>
                </td>
              </tr>
            )}
            {adding === -1 && addPanel(-1)}
            {segs.map((s, k) => {
              const shownItems = items.slice(s.first, s.last + 1).map((x, j) => ({ x, i: s.first + j })).filter(({ x }) => matches(x));
              if (q && shownItems.length === 0) return null;
              const expanded = isOpen(s);
              const errors = shownItems.filter(({ x }) => reports.get(x.key)?.status === "error").length;
              return (
                <Fragment key={items[s.first].key}>
                  <tr
                    data-seg={k}
                    className={`clickable zone-line${focus === k ? " selected" : ""}${drag && drag.over === -(k + 1) ? " drag-below" : ""}`}
                    onClick={(e) => {
                      if (q) return;
                      if (expanded) keepZoneInView(e.currentTarget);
                      toggle(k);
                    }}
                    onDragOver={(e) => {
                      if (!drag) return;
                      e.preventDefault();
                      if (drag.over !== -(k + 1)) setDrag({ ...drag, over: -(k + 1) });
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      if (drag) move(drag.from, s.last);
                      setDrag(null);
                    }}
                  >
                    <td />
                    <td className="num">{fmtTime(s.start)}</td>
                    <td className="num">{duration(s.end - s.start)}</td>
                    <td>{s.fromLevel === s.toLevel ? s.fromLevel : `${s.fromLevel} → ${s.toLevel}`}</td>
                    <td colSpan={2}>
                      <button className="disclosure" aria-expanded={expanded} aria-label={expanded ? t("Replier") : t("Déplier")} disabled={!!q}>
                        {expanded ? "▾" : "▸"}
                      </button>{" "}
                      {s.dungeon && <span className="kind kind-dungeon">{t("Donjon")}</span>} {s.zone ?? <em className="muted">{t("Préparation")}</em>}{" "}
                      <span className="muted">· {t("{n} étapes", { n: steps_(s) })}</span>
                      {errors > 0 && <span className="tag invalid">{t("{n} impossible(s)", { n: errors })}</span>}
                    </td>
                    <td className="row-actions">
                      {moving !== null && moving !== s.last && (
                        <button
                          className="link"
                          onClick={(e) => {
                            e.stopPropagation();
                            move(moving, s.last);
                          }}
                        >
                          {t("Placer à la fin")}
                        </button>
                      )}
                    </td>
                  </tr>
                  {expanded &&
                    shownItems.map(({ x, i }) => (
                      <Fragment key={x.key}>
                        <EditRow
                          index={i}
                          segment={k}
                          step={x.step}
                          report={reports.get(x.key)}
                          label={label(x.step)}
                          moving={moving}
                          active={active === i + 1}
                          onActive={(on) => setActive(on ? i + 1 : null)}
                          dragOver={drag && drag.over === i && drag.from !== i ? (drag.from > i ? "above" : "below") : null}
                          draggable={!q}
                          onDragStart={() => setDrag({ from: i, over: null })}
                          onDragOver={() => {
                            if (drag && drag.over !== i) setDrag({ ...drag, over: i });
                          }}
                          onDrop={() => {
                            // Dropped on a step: it takes its place (before it going up, after it going down).
                            if (drag) move(drag.from, drag.from > i ? i - 1 : i);
                            setDrag(null);
                          }}
                          onDragEnd={() => setDrag(null)}
                          onUp={() => move(i, i - 2)}
                          onDown={() => move(i, i + 1)}
                          onCut={() => setMoving(moving === i ? null : i)}
                          onPlace={() => moving !== null && move(moving, i)}
                          onAdd={() => setAdding(adding === i ? null : i)}
                          onRemove={() => remove(i)}
                          onReplace={(next) => replace(i, next)}
                          last={i === items.length - 1}
                        />
                        {adding === i && addPanel(i)}
                      </Fragment>
                    ))}
                </Fragment>
              );
            })}
          </tbody>
        </table>
        {wide && (
          <aside className="zone-panel">
            {focused ? (
              <ZoneMap title={focused.zone ?? t("Préparation")} steps={mapSteps} active={active} onActive={setActive} />
            ) : (
              <p className="muted">{t("Ouvrez une zone pour voir ses étapes sur sa carte.")}</p>
            )}
          </aside>
        )}
      </div>
    </div>
  );
}

/** Columns of the editor's table. */
const COLUMNS = 7;

interface RowProps {
  index: number;
  /** Its segment (the zone map follows the scroll). */
  segment: number;
  step: RouteStep;
  /** Its replay (the last one while the draft is replayed). */
  report?: StepReport;
  label: string;
  moving: number | null;
  /** Pointed at, here or on the map. */
  active: boolean;
  onActive: (on: boolean) => void;
  /** A step dragged over it lands above or below it. */
  dragOver: "above" | "below" | null;
  draggable: boolean;
  last: boolean;
  onDragStart: () => void;
  onDragOver: () => void;
  onDrop: () => void;
  onDragEnd: () => void;
  onUp: () => void;
  onDown: () => void;
  onCut: () => void;
  onPlace: () => void;
  onAdd: () => void;
  onRemove: () => void;
  onReplace: (step: RouteStep) => void;
}

/** A step of the draft: its line in the guide, the mobs killed on the way, and how to change it. */
function EditRow(p: RowProps) {
  const t = useT();
  const { step, report: r, index: i } = p;
  const kind = KINDS[step.kind];
  const main = r?.line;
  const status = r?.status;
  const reason = r?.reason ? t(REASONS[r.reason] ?? r.reason) : null;
  const classes = ["edit-row", status === "error" && "invalid", status === "skipped" && "skipped", p.moving === i && "moving", p.dragOver && `drag-${p.dragOver}`, p.active && "active"];
  return (
    <tr
      id={`edit-step-${i}`}
      data-seg={p.segment}
      className={classes.filter(Boolean).join(" ")}
      onMouseEnter={() => p.onActive(true)}
      onMouseLeave={() => p.onActive(false)}
      onDragOver={(e) => {
        if (!p.draggable) return;
        e.preventDefault();
        p.onDragOver();
      }}
      onDrop={(e) => {
        e.preventDefault();
        p.onDrop();
      }}
    >
      <td
        className="handle"
        draggable={p.draggable}
        onDragStart={(e) => {
          e.dataTransfer.effectAllowed = "move";
          e.dataTransfer.setData("text/plain", String(i));
          p.onDragStart();
        }}
        onDragEnd={p.onDragEnd}
        title={p.draggable ? t("Glisser pour déplacer") : undefined}
        aria-hidden
      >
        ⠿
      </td>
      <td className="num">{r ? fmtTime(r.start) : "…"}</td>
      <td className={`num${r && r.end - r.start >= 1800 ? " long" : ""}`}>{r && r.end - r.start >= 30 ? fmtTime(r.end - r.start) : ""}</td>
      <td>{r && r.level > 0 ? r.level : ""}</td>
      <td className="num step-n">{i + 1}</td>
      <td>
        <div className="edit-line main">
          <span className={`kind kind-${kind.css}`}>{t(kind.label)}</span>
          <span>
            {main?.text ?? p.label}
            {main?.zone && <span className="muted"> · {main.zone}</span>}
            {r && r.status !== "error" && r.xp > 0 && <span className="muted"> · +{r.xp.toLocaleString()} xp</span>}
          </span>
        </div>
        {reason && <div className={`edit-note ${status === "error" ? "step-error" : "step-skipped"}`}>{status === "error" ? t("Impossible : {reason}", { reason }) : t("Rien à faire : {reason}", { reason })}</div>}
        {r?.farm && step.kind !== "objective" && (
          <div className="edit-note">
            <FarmText farm={r.farm} />
          </div>
        )}
        {step.kind === "objective" && (
          <label className="edit-note farm-toggle">
            <input type="checkbox" checked={!!step.farm} onChange={(e) => p.onReplace({ ...step, farm: e.target.checked })} />
            {t("Tuer tous les mobs croisés en chemin")}
            <span className="muted">
              {step.farm ? r?.farm ? <FarmText farm={r.farm} /> : r ? t("(aucun mob en chemin)") : "" : r?.farm_all ? <FarmText farm={r.farm_all} estimate /> : r ? t("(aucun mob de plus en chemin)") : ""}
            </span>
          </label>
        )}
        {step.kind === "grind" && (
          <label className="edit-note farm-toggle">
            {t("Mobs à tuer")}
            <NumberInput value={step.mobs} min={1} max={2000} onChange={(mobs) => mobs >= 1 && p.onReplace({ kind: "grind", mobs: Math.round(mobs) })} aria-label={t("Mobs à tuer")} />
          </label>
        )}
        {step.kind === "grind_to" && (
          <label className="edit-note farm-toggle">
            {t("Niveau à atteindre")}
            <NumberInput value={step.level} min={2} max={70} onChange={(level) => level >= 2 && p.onReplace({ kind: "grind_to", level: Math.round(level) })} aria-label={t("Niveau à atteindre")} />
          </label>
        )}
      </td>
      <td className="row-actions edit-actions">
        {p.moving !== null && p.moving !== i ? (
          <button className="link" onClick={p.onPlace}>
            {t("Placer ici")}
          </button>
        ) : (
          <>
            <button className="icon" onClick={p.onUp} disabled={i === 0} title={t("Monter")} aria-label={t("Monter")}>
              ↑
            </button>
            <button className="icon" onClick={p.onDown} disabled={p.last} title={t("Descendre")} aria-label={t("Descendre")}>
              ↓
            </button>
            <button className="icon" onClick={p.onCut} title={t("Déplacer ailleurs")} aria-label={t("Déplacer ailleurs")} aria-pressed={p.moving === i}>
              ⇅
            </button>
            <button className="icon" onClick={p.onAdd} title={t("Ajouter une étape après")} aria-label={t("Ajouter une étape après")}>
              +
            </button>
            <button className="icon danger" onClick={p.onRemove} title={t("Supprimer l'étape")} aria-label={t("Supprimer l'étape")}>
              ×
            </button>
          </>
        )}
      </td>
    </tr>
  );
}

/** Mobs killed on the way: count, XP and time (`estimate`: what checking the box would give). */
function FarmText({ farm, estimate }: { farm: Farmed; estimate?: boolean }) {
  const t = useT();
  const vars = { n: farm.kills, xp: farm.xp.toLocaleString(), time: fmtTime(farm.seconds) };
  return <>{estimate ? t("si coché : ~{n} mobs, ~{xp} xp, {time}", vars) : t("{n} mobs tués en chemin : {xp} xp, {time}", vars)}</>;
}

/** Names of the catalog by ID, and the label of a step that has no line of the guide (not replayed). */
function useNames(catalog: EditCatalog | undefined) {
  return useMemo(() => {
    const quests = new Map(catalog?.quests.map((q) => [q.id, q]));
    const nodes = new Map(catalog?.nodes.map((n) => [n.id, n]));
    const inns = new Map(catalog?.inns.map((n) => [n.id, n]));
    const trainers = new Map(catalog?.trainers.map((n) => [n.id, n]));
    const dungeons = new Map(catalog?.dungeons.map((d) => [d.id, d]));
    const quest = (id: number) => quests.get(id)?.name ?? `#${id}`;
    const node = (id: number) => nodes.get(id)?.name ?? `#${id}`;
    const label = (s: RouteStep, t: (fr: string, vars?: Record<string, string | number>) => string): string => {
      switch (s.kind) {
        case "accept":
          return quest(s.quest);
        case "objective":
          return `${quest(s.quest)} — ${quests.get(s.quest)?.objectives[s.objective] ?? `#${s.objective + 1}`}`;
        case "turn_in":
          return quest(s.quest);
        case "flight_master":
          return t("Apprendre le point de vol {name}", { name: node(s.node) });
        case "bind": {
          const inn = inns.get(s.inn);
          return t("Lier la pierre de foyer : {name} ({zone})", { name: inn?.name ?? `#${s.inn}`, zone: inn?.zone ?? "?" });
        }
        case "hearth":
          return s.recall ? t("Rappel astral") : t("Pierre de foyer");
        case "fly":
          return t("Vol {from} → {to}", { from: node(s.from), to: node(s.to) });
        case "dungeon":
          return dungeons.get(s.dungeon)?.name ?? `#${s.dungeon}`;
        case "link":
          return s.first.name === s.last.name && s.first.to === s.last.to ? s.first.name : t("{first}, puis {last}", { first: s.first.name, last: s.last.name });
        case "teleport":
          return s.spell;
        case "train": {
          const trainer = trainers.get(s.trainer);
          return t("S'entraîner auprès de {name} ({zone})", { name: trainer?.name ?? `#${s.trainer}`, zone: trainer?.zone ?? "?" });
        }
        case "abandon":
          return t("Abandonner {quest}", { quest: quest(s.quest) });
        case "grind":
          return t("Tuer {n} mobs dans le coin", { n: s.mobs });
        case "grind_to":
          return t("Farmer jusqu'au niveau {level}", { level: s.level });
      }
    };
    return { label, quests, nodes, dungeons };
  }, [catalog]);
}

interface AddProps {
  /** Inserted after this step (-1: first), where the character has this level. */
  at: number;
  level: number;
  steps: RouteStep[];
  catalog: EditCatalog;
  names: ReturnType<typeof useNames>;
  onAdd: (step: RouteStep) => void;
  onClose: () => void;
}

/** What can be added after a step: a quest to take, do or turn in, the hearthstone, a flight, mobs to kill. */
function AddPanel({ at, level, steps, catalog, names, onAdd, onClose }: AddProps) {
  const t = useT();
  const [kind, setKind] = useState<AddKind>("accept");
  const [search, setSearch] = useState("");
  const [farm, setFarm] = useState(false);
  const [mobs, setMobs] = useState(10);
  const [toLevel, setToLevel] = useState(level + 1);
  const [from, setFrom] = useState<number>(catalog.nodes[0]?.id ?? 0);
  const [to, setTo] = useState<number>(catalog.nodes[1]?.id ?? 0);
  const q = search.trim().toLowerCase();
  const match = (...texts: (string | number)[]) => !q || texts.some((x) => String(x).toLowerCase().includes(q));

  // Quests the draft takes, does and turns in.
  const taken = new Set(steps.flatMap((s) => (s.kind === "accept" ? [s.quest] : [])));
  const turned = new Set(steps.flatMap((s) => (s.kind === "turn_in" ? [s.quest] : [])));
  const done = new Set(steps.flatMap((s) => (s.kind === "objective" ? [`${s.quest}/${s.objective}`] : [])));

  const questRow = (id: number, extra: string | null, add: () => void, disabled = false, note?: string) => {
    const quest = names.quests.get(id);
    return (
      <li key={`${id}/${extra ?? ""}`}>
        <button className="link" onClick={add} disabled={disabled}>
          {quest?.name ?? `#${id}`}
        </button>
        {extra && <span> — {extra}</span>}
        {quest && (
          <span className="muted">
            {" "}
            · {t("niveau {level}", { level: quest.level })} · {quest.zone}
          </span>
        )}
        {note && <span className="tag">{note}</span>}
      </li>
    );
  };

  let body;
  switch (kind) {
    case "accept": {
      // Those of the character's level there first, those the guide already takes last.
      const list = catalog.quests
        .filter((x) => match(x.name, x.zone, x.id))
        .sort((a, b) => Number(taken.has(a.id)) - Number(taken.has(b.id)) || Math.abs(a.level - level) - Math.abs(b.level - level) || a.level - b.level)
        .slice(0, LIST_MAX);
      body = <ul className="pick-list">{list.map((x) => questRow(x.id, null, () => onAdd({ kind: "accept", quest: x.id }), taken.has(x.id), taken.has(x.id) ? t("déjà dans le guide") : undefined))}</ul>;
      break;
    }
    case "objective": {
      const list = [...taken].flatMap((id) =>
        (names.quests.get(id)?.objectives ?? []).flatMap((text, k) => (done.has(`${id}/${k}`) ? [] : [{ id, k, text }])),
      );
      const shown = list.filter((x) => match(names.quests.get(x.id)?.name ?? "", x.text, x.id));
      body = (
        <>
          <label className="farm-toggle">
            <input type="checkbox" checked={farm} onChange={(e) => setFarm(e.target.checked)} /> {t("Tuer tous les mobs croisés en chemin")}{" "}
            <span className="muted">{t("(l'XP estimée s'affiche sur l'étape)")}</span>
          </label>
          {shown.length === 0 ? (
            <p className="muted">{t("Aucun objectif à ajouter : chaque quête prise par le guide a déjà ses objectifs.")}</p>
          ) : (
            <ul className="pick-list">{shown.slice(0, LIST_MAX).map((x) => questRow(x.id, x.text, () => onAdd({ kind: "objective", quest: x.id, objective: x.k, farm: farm || undefined })))}</ul>
          )}
        </>
      );
      break;
    }
    case "turn_in": {
      const list = [...taken].filter((id) => !turned.has(id) && match(names.quests.get(id)?.name ?? "", id));
      body =
        list.length === 0 ? (
          <p className="muted">{t("Aucune quête à rendre : chaque quête prise par le guide est déjà rendue.")}</p>
        ) : (
          <ul className="pick-list">{list.slice(0, LIST_MAX).map((id) => questRow(id, null, () => onAdd({ kind: "turn_in", quest: id })))}</ul>
        );
      break;
    }
    case "bind": {
      const list = catalog.inns.filter((x) => match(x.name, x.zone));
      body = (
        <ul className="pick-list">
          {list.slice(0, LIST_MAX).map((x) => (
            <li key={x.id}>
              <button className="link" onClick={() => onAdd({ kind: "bind", inn: x.id })}>
                {x.name}
              </button>{" "}
              <span className="muted">· {x.zone}</span>
            </li>
          ))}
        </ul>
      );
      break;
    }
    case "hearth":
      body = (
        <div className="actions">
          <button onClick={() => onAdd({ kind: "hearth" })}>{t("Utiliser la pierre de foyer")}</button>
          {catalog.recall && <button onClick={() => onAdd({ kind: "hearth", recall: true })}>{t("Utiliser le Rappel astral")}</button>}
        </div>
      );
      break;
    case "fly":
      body = (
        <div className="fly-pick">
          <label>
            {t("Départ")}
            <NodeSelect nodes={catalog.nodes} value={from} onChange={setFrom} />
          </label>
          <label>
            {t("Arrivée")}
            <NodeSelect nodes={catalog.nodes} value={to} onChange={setTo} />
          </label>
          <button onClick={() => onAdd({ kind: "fly", from, to })} disabled={from === to}>
            {t("Ajouter le vol")}
          </button>
          <p className="muted">{t("Les deux points de vol doivent être connus à ce moment du guide.")}</p>
        </div>
      );
      break;
    case "flight_master": {
      const list = catalog.nodes.filter((x) => match(x.name, x.zone));
      body = (
        <ul className="pick-list">
          {list.map((x) => (
            <li key={x.id}>
              <button className="link" onClick={() => onAdd({ kind: "flight_master", node: x.id })}>
                {x.name}
              </button>
            </li>
          ))}
        </ul>
      );
      break;
    }
    case "train": {
      const list = catalog.trainers.filter((x) => match(x.name, x.zone));
      body = (
        <ul className="pick-list">
          {list.map((x) => (
            <li key={x.id}>
              <button className="link" onClick={() => onAdd({ kind: "train", trainer: x.id })}>
                {x.name}
              </button>{" "}
              <span className="muted">· {x.zone}</span>
            </li>
          ))}
        </ul>
      );
      break;
    }
    case "abandon": {
      const list = [...taken].filter((id) => !turned.has(id) && match(names.quests.get(id)?.name ?? "", id));
      body =
        list.length === 0 ? (
          <p className="muted">{t("Aucune quête à abandonner : chaque quête prise par le guide est déjà rendue.")}</p>
        ) : (
          <ul className="pick-list">{list.slice(0, LIST_MAX).map((id) => questRow(id, null, () => onAdd({ kind: "abandon", quest: id })))}</ul>
        );
      break;
    }
    case "grind":
      body = (
        <div className="fly-pick">
          <label>
            {t("Mobs à tuer")}
            <NumberInput value={mobs} min={1} max={2000} onChange={(n) => setMobs(Math.max(1, Math.round(n)))} aria-label={t("Mobs à tuer")} />
          </label>
          <button onClick={() => onAdd({ kind: "grind", mobs })}>{t("Ajouter")}</button>
          <label>
            {t("Niveau à atteindre")}
            <NumberInput value={toLevel} min={2} max={70} onChange={(n) => setToLevel(Math.max(2, Math.round(n)))} aria-label={t("Niveau à atteindre")} />
          </label>
          <button onClick={() => onAdd({ kind: "grind_to", level: toLevel })}>{t("Ajouter")}</button>
          <p className="muted">{t("Des mobs du niveau du personnage, là où il se trouve : l'XP gagnée s'affiche sur l'étape.")}</p>
        </div>
      );
      break;
    case "dungeon": {
      const list = catalog.dungeons.filter((d) => match(d.name));
      body = (
        <ul className="pick-list">
          {list.map((d) => (
            <li key={d.id}>
              <button className="link" onClick={() => onAdd({ kind: "dungeon", dungeon: d.id })}>
                {d.name}
              </button>{" "}
              <span className="muted">
                · {d.min_level}–{d.max_level}
              </span>
            </li>
          ))}
        </ul>
      );
      break;
    }
  }
  const searchable = kind !== "hearth" && kind !== "fly" && kind !== "grind";
  return (
    <section className="panel add-panel">
      <div className="add-head">
        <h3>{at < 0 ? t("Ajouter une étape au début") : t("Ajouter une étape après l'étape {n}", { n: at + 1 })}</h3>
        <button className="link" onClick={onClose}>
          {t("Fermer")}
        </button>
      </div>
      <div className="add-kinds" role="tablist">
        {ADD_KINDS.filter((k) => k.kind !== "dungeon" || catalog.dungeons.length > 0).map((k) => (
          <button key={k.kind} role="tab" aria-selected={kind === k.kind} className={kind === k.kind ? "active" : ""} onClick={() => setKind(k.kind)}>
            {t(k.label)}
          </button>
        ))}
      </div>
      {searchable && <input className="search" placeholder={t("Rechercher…")} value={search} onChange={(e) => setSearch(e.target.value)} autoFocus />}
      {body}
    </section>
  );
}

function NodeSelect({ nodes, value, onChange }: { nodes: EditCatalog["nodes"]; value: number; onChange: (id: number) => void }) {
  return (
    <select value={value} onChange={(e) => onChange(Number(e.target.value))}>
      {nodes.map((n) => (
        <option key={n.id} value={n.id}>
          {n.name}
        </option>
      ))}
    </select>
  );
}
