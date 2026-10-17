import { memo, useEffect, useMemo, useRef, useState } from "react";
import type { QuestInfo, RouteStep, Step, StepReport } from "../api";
import { useT } from "../i18n";
import { FarmText, REASONS } from "./editing";
import { NumberInput } from "./NumberInput";
import { type Card, fmtClock, isCheckpoint, questOf, type Spans, splitOf } from "./guideTimeline";
import type { Segment } from "./segments";
import { KIND_LABELS } from "./segments";

/** Lines a card shows before "+ n more" (the card under way shows them all). */
const SHOWN_LINES = 3;

interface Props {
  steps: Step[];
  cards: Card[];
  titles: string[];
  /** The zones (a dungeon run apart), by steps. */
  segments: Segment[];
  /** The timeline of each step (none while the replay loads). */
  spans: Spans;
  quests: Map<number, QuestInfo>;
  /** Card under way. */
  current: number;
  onSelect: (card: number) => void;
  /** Cards checked for a problem report, and checking one (a box on each card when given). */
  checked?: Set<number>;
  onCheck?: (card: number) => void;
  /** Editing: what each line is in the draft, and how to change it. */
  edit?: ListEdit;
}

/** The draft behind the lines of the list being edited, a line per step. */
export interface ListEdit {
  steps: RouteStep[];
  /** The replay of each step (none: not replayed yet), and those changed since the replay shown. */
  reports: (StepReport | undefined)[];
  pending: boolean[];
  /** Move the lines `first..=last` before line `to` (the number of lines: at the end). */
  onMove: (first: number, last: number, to: number) => void;
  onRemove: (i: number) => void;
  onReplace: (i: number, step: RouteStep) => void;
  /** Open the menu to add a step after line `i` (-1: at the start), beside a point of the window. */
  onAdd: (i: number, anchor: { x: number; y: number }) => void;
}

/** Where what is dragged lands: before line `at`. */
interface Drop {
  at: number;
}

/** The lines dragged ("first-last"). */
const DRAG_TYPE = "text/plain";

/** The guide's steps grouped by zone, each place a card: its time, title, lines and where its time goes. */
export function StepList({ steps, cards, titles, segments, spans, quests, current, onSelect, checked, onCheck, edit }: Props) {
  const t = useT();
  const list = useRef<HTMLDivElement>(null);
  const [drop, setDrop] = useState<Drop | null>(null);
  // The cards of each zone.
  const groups = useMemo(() => {
    const out: { seg: Segment; first: number; last: number }[] = [];
    let c = 0;
    for (const seg of segments) {
      const first = c;
      while (c < cards.length && cards[c].first <= seg.last) c++;
      if (c > first) out.push({ seg, first, last: c - 1 });
    }
    return out;
  }, [segments, cards]);

  // The card under way stays in view.
  useEffect(() => {
    list.current?.querySelector<HTMLElement>(`[data-card="${current}"]`)?.scrollIntoView({ block: "nearest" });
  }, [current]);

  return (
    <div className="step-list">
      <div className="sl-head">
        <strong>{t("Étapes")}</strong>
        {edit && (
          <button className="link sl-add-first" onClick={(e) => edit.onAdd(-1, anchorOf(e.currentTarget))} title={t("Ajouter une étape au début")}>
            + {t("au début")}
          </button>
        )}
        <span className="muted">{cards.length}</span>
      </div>
      <div className="sl-legend" aria-hidden>
        <span>{t("Temps :")}</span>
        <span>
          <i className="cat-travel" /> {t("Trajet")}
        </span>
        <span>
          <i className="cat-combat" /> {t("Combat")}
        </span>
        <span>
          <i className="cat-interact" /> {t("Interaction")}
        </span>
        <span>
          <i className="cat-wait" /> {t("Attente")}
        </span>
        <span>≈ {t("Estimé")}</span>
      </div>
      <div
        className={`sl-scroll${edit ? " editing" : ""}`}
        ref={list}
        onDragLeave={(e) => {
          if (!list.current?.contains(e.relatedTarget as Node)) setDrop(null);
        }}
      >
        {groups.map(({ seg, first, last }, g) => (
          <ZoneSection
            key={g}
            seg={seg}
            next={groups[g + 1]?.seg.zone ?? null}
            cards={cards}
            first={first}
            last={last}
            titles={titles}
            steps={steps}
            spans={spans}
            quests={quests}
            current={current >= first && current <= last ? current : -1}
            onSelect={onSelect}
            checked={checked}
            onCheck={onCheck}
            edit={edit}
            drop={drop && drop.at >= cards[first].first && drop.at <= cards[last].last + 1 ? drop : null}
            onDrop={setDrop}
          />
        ))}
      </div>
    </div>
  );
}

interface ZoneProps {
  seg: Segment;
  /** The zone after it. */
  next: string | null;
  cards: Card[];
  first: number;
  last: number;
  titles: string[];
  steps: Step[];
  spans: Spans;
  quests: Map<number, QuestInfo>;
  /** The card under way when it is in this zone, else -1. */
  current: number;
  onSelect: (card: number) => void;
  checked?: Set<number>;
  onCheck?: (card: number) => void;
  edit?: ListEdit;
  /** Where what is dragged lands, when in this zone. */
  drop: Drop | null;
  onDrop: (drop: Drop | null) => void;
}

/** A zone and its cards: drawn again only when the card under way enters or leaves it (or the draft changes). */
const ZoneSection = memo(function ZoneSection({ seg, next, cards, first, last, titles, steps, spans, quests, current, onSelect, checked, onCheck, edit, drop, onDrop }: ZoneProps) {
  const t = useT();
  return (
    <section className="sl-zone">
      <header className="sl-zone-head">
        <strong>{seg.zone ?? t("Préparation")}</strong>
        <small>
          {fmtClock(seg.start)} – {fmtClock(seg.end)} · {t("niv. {from} → {to}", { from: seg.fromLevel, to: seg.toLevel })}
          {next && <span className="sl-next"> → {next}</span>}
        </small>
      </header>
      {cards.slice(first, last + 1).map((c) => (
        <CardView
          key={c.n}
          card={c}
          title={titles[c.n]}
          steps={steps}
          spans={spans}
          quests={quests}
          active={c.n === current}
          onSelect={onSelect}
          checked={checked?.has(c.n)}
          onCheck={onCheck}
          edit={edit}
          drop={drop && drop.at >= c.first && drop.at <= c.last + 1 ? drop : null}
          onDrop={onDrop}
        />
      ))}
    </section>
  );
});

interface CardProps {
  card: Card;
  title: string;
  steps: Step[];
  spans: Spans;
  quests: Map<number, QuestInfo>;
  active: boolean;
  onSelect: (card: number) => void;
  checked?: boolean;
  onCheck?: (card: number) => void;
  edit?: ListEdit;
  drop: Drop | null;
  onDrop: (drop: Drop | null) => void;
}

/** A point of the window beside an element (where a menu opens): its right edge, at its top. */
const anchorOf = (el: Element) => {
  const r = el.getBoundingClientRect();
  return { x: r.right + 8, y: r.top };
};

/** The lines dragged, read back from a drag. */
function dragged(e: React.DragEvent): [number, number] | null {
  const m = /^(\d+)-(\d+)$/.exec(e.dataTransfer.getData(DRAG_TYPE));
  return m ? [Number(m[1]), Number(m[2])] : null;
}

/**
 * Whether a card would be drawn the same: the same card, title and state, the same lines (and,
 * editing, the same steps, replays and pending marks). A long guide has a thousand cards: a
 * change draws again only those it touches.
 */
function sameCard(a: CardProps, b: CardProps): boolean {
  const [x, y] = [a.card, b.card];
  if (
    x.n !== y.n ||
    x.first !== y.first ||
    x.last !== y.last ||
    x.start !== y.start ||
    x.end !== y.end ||
    x.ding !== y.ding ||
    x.farm !== y.farm ||
    a.title !== b.title ||
    a.active !== b.active ||
    a.checked !== b.checked ||
    a.drop?.at !== b.drop?.at ||
    a.quests !== b.quests ||
    a.onSelect !== b.onSelect ||
    a.onCheck !== b.onCheck ||
    a.onDrop !== b.onDrop ||
    !a.spans !== !b.spans ||
    !a.edit !== !b.edit
  )
    return false;
  if (a.edit && b.edit && (a.edit.onMove !== b.edit.onMove || a.edit.onRemove !== b.edit.onRemove || a.edit.onReplace !== b.edit.onReplace || a.edit.onAdd !== b.edit.onAdd)) return false;
  for (let i = x.first; i <= x.last; i++) {
    if (a.steps[i] !== b.steps[i] || a.spans?.[i] !== b.spans?.[i]) return false;
    if (a.edit && b.edit && (a.edit.steps[i] !== b.edit.steps[i] || a.edit.reports[i] !== b.edit.reports[i] || a.edit.pending[i] !== b.edit.pending[i])) return false;
  }
  // The last line edited shows a drop below it.
  return !(a.edit && b.edit && a.edit.steps.length !== b.edit.steps.length && x.last >= Math.min(a.edit.steps.length, b.edit.steps.length) - 1);
}

/** A card: drawn again only when it becomes, or stops being, the one under way (or its lines change, see `sameCard`). */
const CardView = memo(function CardView({ card, title, steps, spans, quests, active, onSelect, checked, onCheck, edit, drop, onDrop }: CardProps) {
  const t = useT();
  const lines = steps.slice(card.first, card.last + 1).map((s, k) => ({ s, i: card.first + k }));
  // Editing: every line, to drag it anywhere.
  const shown = active || edit ? lines : lines.slice(0, SHOWN_LINES);
  const errors = edit ? lines.filter(({ i }) => edit.reports[i]?.status === "error").length : 0;
  const split = splitOf(spans, card.first, card.last);
  const length = Math.max(1, card.end - card.start);
  const estimate = spans?.slice(card.first, card.last + 1).some((s) => s?.estimate);
  return (
    <div
      className={`sl-card${active ? " active" : ""}${card.travel ? " travel" : ""}${errors ? " invalid" : ""}${drop?.at === card.first ? " drop-before" : ""}`}
      data-card={card.n}
      role="button"
      tabIndex={0}
      aria-current={active ? "step" : undefined}
      onClick={() => onSelect(card.n)}
      onKeyDown={(e) => {
        if (e.key === "Enter") onSelect(card.n);
      }}
    >
      <div
        className="sl-card-head"
        draggable={!!edit}
        title={edit ? t("Glisser pour déplacer") : undefined}
        onDragStart={(e) => {
          e.dataTransfer.effectAllowed = "move";
          e.dataTransfer.setData(DRAG_TYPE, `${card.first}-${card.last}`);
        }}
        onDragOver={(e) => {
          if (!edit) return;
          e.preventDefault();
          if (drop?.at !== card.first) onDrop({ at: card.first });
        }}
        onDrop={(e) => {
          e.preventDefault();
          const d = dragged(e);
          if (edit && d) edit.onMove(d[0], d[1], card.first);
          onDrop(null);
        }}
        onDragEnd={() => onDrop(null)}
      >
        <span className="sl-time">{fmtClock(card.start)}</span>
        {onCheck && (
          <input
            type="checkbox"
            className="step-check"
            checked={checked ?? false}
            onClick={(e) => e.stopPropagation()}
            onChange={() => onCheck(card.n)}
            aria-label={t("Joindre l'étape {n} au signalement", { n: card.n + 1 })}
          />
        )}
        <span className="sl-n">{card.n + 1}</span>
        <span className="sl-title">{title}</span>
        {card.ding && <span className="sl-ding">{t("NIV. {n}", { n: card.ding })}</span>}
        <span className="sl-dur" title={estimate ? t("Durée estimée : cette étape n'a pas été retrouvée en rejouant le guide") : undefined}>
          {estimate && "≈ "}
          {fmtClock(card.end - card.start)}
        </span>
      </div>
      <ul className="sl-lines">
        {shown.map(({ s, i }) =>
          edit ? (
            <EditLine key={i} index={i} step={s} quests={quests} edit={edit} drop={drop} onDrop={onDrop} />
          ) : (
            <Line key={i} step={s} quests={quests} />
          ),
        )}
        {shown.length < lines.length && <li className="sl-more">{t("+ {n} de plus", { n: lines.length - shown.length })}</li>}
      </ul>
      {card.farm > 0 && <div className="sl-farm">{t("× {n} mobs tués en chemin", { n: card.farm })}</div>}
      {spans && (
        <div className="sl-bar" aria-hidden>
          {(["travel", "combat", "interact", "wait"] as const).map((k) => split[k] > 0 && <i key={k} className={`cat-${k}`} style={{ width: `${(split[k] / length) * 100}%` }} />)}
        </div>
      )}
    </div>
  );
}, sameCard);

/** Objective kinds as the line says them (French keys of the catalog). */
const OBJECTIVE_LABELS: Record<string, string> = { kill: "Tuer", collect: "Récupérer", use: "Utiliser", talk: "Parler", other: "Faire" };
/** The glyph before each kind of line. */
const GLYPHS: Record<string, string> = { accept: "!", turnin: "?", kill: "✕", collect: "◆", use: "◆", talk: "…", fly: "➜", link: "➜", hearth: "⌂", teleport: "➜" };

/** What a line says: its kind (French key), the kind of its objective, its text. */
function lineOf(s: Step, quests: Map<number, QuestInfo>) {
  const { quest, objective } = questOf(quests, s);
  let kind = s.kind;
  let label = KIND_LABELS[s.kind] ?? s.kind;
  let text = s.text;
  if (s.kind === "accept" || s.kind === "turnin" || s.kind === "abandon") text = quest?.name ?? s.quest_name ?? s.text;
  else if (s.kind === "objective" && objective) {
    kind = objective.kind;
    label = OBJECTIVE_LABELS[objective.kind];
    text = objective.kind === "other" || !s.target ? objective.text : `${s.target.name}${objective.count > 1 ? ` ×${objective.count}` : ""}`;
  }
  return { kind, label, text };
}

/** A line of a card: its kind (colored) and what it is about. */
function Line({ step: s, quests }: { step: Step; quests: Map<number, QuestInfo> }) {
  const t = useT();
  if (isCheckpoint(s)) return <li className="sl-line checkpoint">{s.text}</li>;
  const { kind, label, text } = lineOf(s, quests);
  return (
    <li className={`sl-line kind-${s.kind} obj-${kind}`} title={s.text}>
      <b>
        <span aria-hidden>{GLYPHS[kind] ?? "•"}</span> {t(label)}
      </b>
      <span>{text}</span>
    </li>
  );
}

interface EditLineProps {
  index: number;
  step: Step;
  quests: Map<number, QuestInfo>;
  edit: ListEdit;
  drop: Drop | null;
  onDrop: (drop: Drop | null) => void;
}

/**
 * A line being edited: dragged to move it, why it is not possible (or does nothing), killing
 * every mob on the way to an objective (what it would give before it is checked), the mobs of
 * a grind, and buttons to add a step after it or remove it.
 */
function EditLine({ index: i, step: s, quests, edit, drop, onDrop }: EditLineProps) {
  const t = useT();
  const { kind, label, text } = lineOf(s, quests);
  const r = edit.reports[i];
  const step = edit.steps[i];
  const reason = r?.reason ? t(REASONS[r.reason] ?? r.reason) : null;
  const classes = ["sl-line", "edit", `kind-${s.kind}`, `obj-${kind}`, r?.status === "error" && "invalid", r?.status === "skipped" && "skipped", edit.pending[i] && "pending"];
  const place = drop?.at === i ? " drop-before" : drop?.at === i + 1 && i === edit.steps.length - 1 ? " drop-after" : "";
  return (
    <li
      className={classes.filter(Boolean).join(" ") + place}
      draggable
      title={s.text}
      onDragStart={(e) => {
        e.stopPropagation();
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData(DRAG_TYPE, `${i}-${i}`);
      }}
      onDragOver={(e) => {
        e.preventDefault();
        e.stopPropagation();
        // The upper half: before it, the lower: after it.
        const box = e.currentTarget.getBoundingClientRect();
        const at = e.clientY < box.top + box.height / 2 ? i : i + 1;
        if (drop?.at !== at) onDrop({ at });
      }}
      onDrop={(e) => {
        e.preventDefault();
        e.stopPropagation();
        const d = dragged(e);
        if (d && drop) edit.onMove(d[0], d[1], drop.at);
        onDrop(null);
      }}
      onDragEnd={() => onDrop(null)}
    >
      <div className="sl-edit-main">
        <span className="sl-grip" aria-hidden>
          ⠿
        </span>
        <b>
          <span aria-hidden>{GLYPHS[kind] ?? "•"}</span> {t(label)}
        </b>
        <span className="sl-text">{text}</span>
        <span className="sl-tools">
          <button
            className="icon"
            onClick={(e) => {
              e.stopPropagation();
              edit.onAdd(i, anchorOf(e.currentTarget.closest(".sl-line") ?? e.currentTarget));
            }} title={t("Ajouter une étape après")} aria-label={t("Ajouter une étape après")}>
            +
          </button>
          <button
            className="icon danger"
            onClick={(e) => {
              e.stopPropagation();
              edit.onRemove(i);
            }} title={t("Supprimer l'étape")} aria-label={t("Supprimer l'étape")}>
            ×
          </button>
        </span>
      </div>
      {reason && <div className={r?.status === "error" ? "sl-why error" : "sl-why"}>{r?.status === "error" ? t("Impossible : {reason}", { reason }) : t("Rien à faire : {reason}", { reason })}</div>}
      {step.kind === "objective" && (
        <label className="sl-option" onClick={(e) => e.stopPropagation()}>
          <input type="checkbox" checked={!!step.farm} onChange={(e) => edit.onReplace(i, { ...step, farm: e.target.checked || undefined })} />
          {t("Tuer tous les mobs croisés en chemin")}{" "}
          <span className="muted">{step.farm ? r?.farm ? <FarmText farm={r.farm} /> : "" : r?.farm_all ? <FarmText farm={r.farm_all} estimate /> : ""}</span>
        </label>
      )}
      {step.kind === "grind" && (
        <label className="sl-option" onClick={(e) => e.stopPropagation()}>
          {t("Mobs à tuer")}
          <NumberInput value={step.mobs} min={1} max={2000} onChange={(mobs) => mobs >= 1 && edit.onReplace(i, { kind: "grind", mobs: Math.round(mobs) })} aria-label={t("Mobs à tuer")} />
        </label>
      )}
      {step.kind === "grind_to" && (
        <label className="sl-option" onClick={(e) => e.stopPropagation()}>
          {t("Niveau à atteindre")}
          <NumberInput value={step.level} min={2} max={70} onChange={(level) => level >= 2 && edit.onReplace(i, { kind: "grind_to", level: Math.round(level) })} aria-label={t("Niveau à atteindre")} />
        </label>
      )}
    </li>
  );
}
