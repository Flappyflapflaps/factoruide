import { memo, useCallback, useMemo } from "react";
import type { InnInfo, LegMode, ObjectiveInfo, PlanRequest, QuestInfo, Route, Timeline } from "../api";
import { useGameName, useT } from "../i18n";
import { type CharState, fmtClock, questOf, xpAt } from "./guideTimeline";
import { Money } from "./Money";
import { type Clock, useClock } from "./playback";

interface Props {
  route: Route;
  request: PlanRequest;
  timeline: Timeline | null;
  quests: Map<number, QuestInfo>;
  /** What the character has after the step before the one under way. */
  state: CharState | null;
  clock: Clock;
  /** Step and card under way, and how many cards. */
  step: number;
  card: number;
  cards: number;
  /** How the character moves now (none: it does not). */
  moving: LegMode | null;
}

/** Travel modes as the panel says them (French keys of the catalog). */
const TRAVEL: Record<LegMode | "foot", string> = {
  foot: "À pied",
  walk: "À pied",
  mount: "Monté",
  flight: "En vol",
  link: "En bateau",
  hearth: "Pierre de foyer",
  teleport: "Téléportation",
};

/**
 * The character at the moment watched: level and XP, hearthstone, quest log, items, flight
 * paths, professions. Drawn again when the step changes; what moves with the time (XP, the
 * hearthstone's cooldown, the objective under way) in parts of its own.
 */
export const CharacterPanel = memo(function CharacterPanel({ route, request, timeline, quests, state, clock, step, card, cards, moving }: Props) {
  const t = useT();
  const gameName = useGameName();
  const inns = useMemo(() => new Map<number, InnInfo>((timeline?.inns ?? []).map((i) => [i.id, i])), [timeline]);
  const nodes = useMemo(() => new Map((timeline?.nodes ?? []).map((n) => [n.id, n.name])), [timeline]);
  const s = route.steps[step];
  const working = s?.kind === "objective" ? questOf(quests, s) : null;
  const log = state ? [...state.log] : [];
  const items = log.flatMap(([q, done]) =>
    (quests.get(q)?.objectives ?? []).flatMap((o, k) => (o.kind === "collect" && done.has(k) ? [{ key: `${q}-${k}`, text: o.text }] : [])),
  );
  const inn = state?.bind != null ? inns.get(state.bind) : undefined;
  const travel = moving ?? (s && s.level >= (timeline?.mount_level ?? 99) ? "mount" : "foot");
  const level = Math.max(route.from_level, s?.level ?? route.from_level);
  const professions = (route.professions ?? []).filter((p) => (p.curve[level] ?? 0) > 0);

  return (
    <div className="char-panel">
      <div className="cp-head">
        <strong>{t("Personnage")}</strong>
        <span className="cp-class">
          {gameName(request.race)} {gameName(request.class)}
        </span>
      </div>
      <Experience route={route} timeline={timeline} clock={clock} step={step} />
      <dl className="cp-grid">
        <div>
          <dt>{t("Pierre de foyer")}</dt>
          <dd>
            <Cooldown clock={clock} ready={state?.hearth ?? 0} label={t("Prête")} />
          </dd>
        </div>
        <div>
          <dt>{t("Journal de quêtes")}</dt>
          <dd>
            {log.length} / {timeline?.log_size ?? 20}
          </dd>
        </div>
        <div>
          <dt>{t("Lié à")}</dt>
          <dd title={inn?.name}>{inn?.place ?? "—"}</dd>
        </div>
        <div>
          <dt>{t("Déplacement")}</dt>
          <dd>{t(TRAVEL[travel])}</dd>
        </div>
        {timeline?.start.recall && (
          <div>
            <dt>{t("Rappel astral")}</dt>
            <dd>
              <Cooldown clock={clock} ready={state?.recall ?? 0} label={t("Prêt")} />
            </dd>
          </div>
        )}
        {s?.power != null && (
          <div title={t("Niveau, plus ce que l'équipement et les sorts apportent")}>
            <dt>{t("Puissance")}</dt>
            <dd>
              {s.power.toFixed(1)}
              {s.gear ? <small className="muted"> ({t("équipement +{n}", { n: s.gear.toFixed(1) })})</small> : null}
            </dd>
          </div>
        )}
        {state && state.cost > 0 && (
          <div>
            <dt>{t("Vols payés")}</dt>
            <dd>
              <Money copper={state.cost} />
            </dd>
          </div>
        )}
        {professions.map((p) => (
          <div key={p.key}>
            <dt>{p.name}</dt>
            <dd>{p.curve[level]}</dd>
          </div>
        ))}
      </dl>
      <details className="cp-section" open>
        <summary>
          {t("Dans le journal")} <span className="muted">{log.length}</span>
        </summary>
        {log.length === 0 && <p className="muted">{t("Aucune quête.")}</p>}
        <ul className="cp-log">
          {log.map(([q, done]) => {
            const quest = quests.get(q);
            const active = working?.quest?.id === q;
            return (
              <li key={q} className={active ? "active" : undefined}>
                <span className="cp-quest">{quest?.name ?? q}</span>
                {quest?.objectives.map((o, k) =>
                  done.has(k) ? (
                    <small key={k} className="done">
                      ✓ {o.text}
                    </small>
                  ) : active && s.objective === k && timeline ? (
                    <Progress key={k} clock={clock} objective={o} span={timeline.steps[step]} />
                  ) : (
                    <small key={k}>
                      0/{o.count} {uncounted(o.text)}
                    </small>
                  ),
                )}
              </li>
            );
          })}
        </ul>
      </details>
      <details className="cp-section" open>
        <summary>
          {t("Objets de quête")} <span className="muted">{items.length}</span>
        </summary>
        {items.length === 0 ? (
          <p className="muted">{t("Aucun objet de quête.")}</p>
        ) : (
          <ul className="cp-items">
            {items.map((i) => (
              <li key={i.key}>{i.text}</li>
            ))}
          </ul>
        )}
      </details>
      <details className="cp-section">
        <summary>
          {t("Points de vol")} <span className="muted">{state?.flights.size ?? 0}</span>
        </summary>
        <ul className="cp-items">
          {[...(state?.flights ?? [])].map((f) => (
            <li key={f}>{nodes.get(f) ?? f}</li>
          ))}
        </ul>
      </details>
      <Played clock={clock} total={route.total_time} card={card} cards={cards} />
    </div>
  );
});

/** An objective's text without its count ("Kill Night Web Spider x8": the count is shown apart). */
const uncounted = (text: string) => text.replace(/\s+x\d+$/, "");

/** Level and XP at the moment watched, its bar. */
function Experience({ route, timeline, clock, step }: { route: Route; timeline: Timeline | null; clock: Clock; step: number }) {
  const t = useT();
  const time = useClock(
    clock,
    useCallback((c: Clock) => Math.floor(c.time), []),
  );
  const xp = xpAt(route, timeline, step, time);
  const level = Math.max(route.from_level, xp.level);
  return (
    <>
      <div className="cp-level">
        <strong>{t("Niveau {n}", { n: level })}</strong>
        {xp.next > 0 && (
          <span className="muted">
            {xp.xp.toLocaleString()} / {xp.next.toLocaleString()} XP ({Math.floor((xp.xp / xp.next) * 100)} %)
          </span>
        )}
      </div>
      <div className="cp-xp" role="progressbar" aria-valuemin={0} aria-valuemax={xp.next} aria-valuenow={xp.xp} aria-label={t("Expérience")}>
        <i style={{ width: `${xp.next ? (xp.xp / xp.next) * 100 : 100}%` }} />
      </div>
    </>
  );
}

/** Ready (`label`), or the time left before it is. */
function Cooldown({ clock, ready, label }: { clock: Clock; ready: number; label: string }) {
  const left = useClock(
    clock,
    useCallback((c: Clock) => Math.max(0, Math.ceil(ready - c.time)), [ready]),
  );
  return <>{left > 0 ? fmtClock(left) : label}</>;
}

/** The objective under way, counted up as its time goes. */
function Progress({ clock, objective, span }: { clock: Clock; objective: ObjectiveInfo; span: { arrive: number; end: number } }) {
  const done = useClock(
    clock,
    useCallback(
      (c: Clock) => (c.time < span.arrive ? 0 : Math.min(objective.count - 1, Math.floor(((c.time - span.arrive) / Math.max(1, span.end - span.arrive)) * objective.count))),
      [objective, span],
    ),
  );
  return (
    <small className="now">
      {done}/{objective.count} {uncounted(objective.text)}
    </small>
  );
}

/** Time played and the card under way. */
function Played({ clock, total, card, cards }: { clock: Clock; total: number; card: number; cards: number }) {
  const t = useT();
  const time = useClock(
    clock,
    useCallback((c: Clock) => Math.floor(c.time), []),
  );
  return <p className="cp-foot muted">{t("Joué {time} sur {total} · étape {n} sur {count}", { time: fmtClock(time), total: fmtClock(total), n: card + 1, count: cards })}</p>;
}
