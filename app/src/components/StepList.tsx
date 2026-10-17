import { memo, useEffect, useMemo, useRef } from "react";
import type { QuestInfo, Step, Timeline } from "../api";
import { useT } from "../i18n";
import { type Card, fmtClock, isCheckpoint, questOf, splitOf } from "./guideTimeline";
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
  timeline: Timeline | null;
  quests: Map<number, QuestInfo>;
  /** Card under way. */
  current: number;
  onSelect: (card: number) => void;
  /** Cards checked for a problem report, and checking one (a box on each card when given). */
  checked?: Set<number>;
  onCheck?: (card: number) => void;
}

/** The guide's steps grouped by zone, each place a card: its time, title, lines and where its time goes. */
export function StepList({ steps, cards, titles, segments, timeline, quests, current, onSelect, checked, onCheck }: Props) {
  const t = useT();
  const list = useRef<HTMLDivElement>(null);
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
      <div className="sl-scroll" ref={list}>
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
            timeline={timeline}
            quests={quests}
            current={current >= first && current <= last ? current : -1}
            onSelect={onSelect}
            checked={checked}
            onCheck={onCheck}
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
  timeline: Timeline | null;
  quests: Map<number, QuestInfo>;
  /** The card under way when it is in this zone, else -1. */
  current: number;
  onSelect: (card: number) => void;
  checked?: Set<number>;
  onCheck?: (card: number) => void;
}

/** A zone and its cards: drawn again only when the card under way enters or leaves it. */
const ZoneSection = memo(function ZoneSection({ seg, next, cards, first, last, titles, steps, timeline, quests, current, onSelect, checked, onCheck }: ZoneProps) {
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
          timeline={timeline}
          quests={quests}
          active={c.n === current}
          onSelect={onSelect}
          checked={checked?.has(c.n)}
          onCheck={onCheck}
        />
      ))}
    </section>
  );
});

interface CardProps {
  card: Card;
  title: string;
  steps: Step[];
  timeline: Timeline | null;
  quests: Map<number, QuestInfo>;
  active: boolean;
  onSelect: (card: number) => void;
  checked?: boolean;
  onCheck?: (card: number) => void;
}

/** A card: drawn again only when it becomes, or stops being, the one under way. */
const CardView = memo(function CardView({ card, title, steps, timeline, quests, active, onSelect, checked, onCheck }: CardProps) {
  const t = useT();
  const lines = steps.slice(card.first, card.last + 1).map((s, k) => ({ s, i: card.first + k }));
  const shown = active ? lines : lines.slice(0, SHOWN_LINES);
  const split = splitOf(timeline, card.first, card.last);
  const length = Math.max(1, card.end - card.start);
  const estimate = timeline?.steps.slice(card.first, card.last + 1).some((s) => s.estimate);
  return (
    <div
      className={`sl-card${active ? " active" : ""}${card.travel ? " travel" : ""}`}
      data-card={card.n}
      role="button"
      tabIndex={0}
      aria-current={active ? "step" : undefined}
      onClick={() => onSelect(card.n)}
      onKeyDown={(e) => {
        if (e.key === "Enter") onSelect(card.n);
      }}
    >
      <div className="sl-card-head">
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
        {shown.map(({ s, i }) => (
          <Line key={i} step={s} quests={quests} />
        ))}
        {shown.length < lines.length && <li className="sl-more">{t("+ {n} de plus", { n: lines.length - shown.length })}</li>}
      </ul>
      {card.farm > 0 && <div className="sl-farm">{t("× {n} mobs tués en chemin", { n: card.farm })}</div>}
      {timeline && (
        <div className="sl-bar" aria-hidden>
          {(["travel", "combat", "interact", "wait"] as const).map((k) => split[k] > 0 && <i key={k} className={`cat-${k}`} style={{ width: `${(split[k] / length) * 100}%` }} />)}
        </div>
      )}
    </div>
  );
});

/** Objective kinds as the line says them (French keys of the catalog). */
const OBJECTIVE_LABELS: Record<string, string> = { kill: "Tuer", collect: "Récupérer", use: "Utiliser", talk: "Parler", other: "Faire" };
/** The glyph before each kind of line. */
const GLYPHS: Record<string, string> = { accept: "!", turnin: "?", kill: "✕", collect: "◆", use: "◆", talk: "…", fly: "➜", link: "➜", hearth: "⌂", teleport: "➜" };

/** A line of a card: its kind (colored) and what it is about. */
function Line({ step: s, quests }: { step: Step; quests: Map<number, QuestInfo> }) {
  const t = useT();
  if (isCheckpoint(s)) return <li className="sl-line checkpoint">{s.text}</li>;
  const { quest, objective } = questOf(quests, s);
  let kind = s.kind;
  let label = t(KIND_LABELS[s.kind] ?? s.kind);
  let text = s.text;
  if (s.kind === "accept" || s.kind === "turnin" || s.kind === "abandon") text = quest?.name ?? s.quest_name ?? s.text;
  else if (s.kind === "objective" && objective) {
    kind = objective.kind;
    label = t(OBJECTIVE_LABELS[objective.kind]);
    text = objective.kind === "other" || !s.target ? objective.text : `${s.target.name}${objective.count > 1 ? ` ×${objective.count}` : ""}`;
  }
  return (
    <li className={`sl-line kind-${s.kind} obj-${kind}`} title={s.text}>
      <b>
        <span aria-hidden>{GLYPHS[kind] ?? "•"}</span> {label}
      </b>
      <span>{text}</span>
    </li>
  );
}
