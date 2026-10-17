import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { api, type GuideVersion, type QuestInfo, type Timeline } from "../api";
import { gameLocale, useLang, useT } from "../i18n";
import { CharacterPanel } from "./CharacterPanel";
import { DEFAULT_LAYERS, GuideMap, type MapLayers } from "./GuideMap";
import { activityAt, buildCards, type Card, cardOf, cardTitle, positionAt, stateTracker, stepAt, straightMoves, travelEnds } from "./guideTimeline";
import { PlaybackBar } from "./PlaybackBar";
import { Clock, useClock } from "./playback";
import { segments } from "./segments";
import { StepList } from "./StepList";

const LAYERS_KEY = "factoruide.mapLayers";

interface Props {
  version: GuideVersion;
  /** Edit the guide's steps by hand. */
  onEditSteps?: () => void;
  /** Steps checked for a problem report (indexes), and checking those of a card (a box on each card when given). */
  checked?: Set<number>;
  onCheck?: (steps: number[]) => void;
}

/**
 * A guide watched as it is played: its steps by place on the left, the map in the middle with
 * the character moving along the route, the character's state on the right, and the time at
 * the bottom. The steps, the map and the time follow each other.
 */
export function GuideViewer({ version, onEditSteps, checked, onCheck }: Props) {
  const t = useT();
  const lang = useLang();
  const { meta, outcome } = version;
  const route = outcome.route;
  const steps = route.steps;

  // The replay comes a little after the guide: straight lines between its steps meanwhile.
  const [timeline, setTimeline] = useState<Timeline | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    api
      .guideTimeline(meta.guide, meta.version, gameLocale(lang))
      .then((tl) => live && setTimeline(tl))
      .catch((e) => live && setFailed(String(e)));
    return () => {
      live = false;
    };
  }, [meta.guide, meta.version, lang]);

  const cards = useMemo(() => buildCards(steps, route.from_level), [steps, route.from_level]);
  const titles = useMemo(() => cards.map((c) => cardTitle(c, steps, timeline)), [cards, steps, timeline]);
  const segs = useMemo(() => segments(steps), [steps]);
  const quests = useMemo(() => new Map((timeline?.quests ?? []).map((q) => [q.id, q])), [timeline]);
  const moves = useMemo(() => (timeline ? { start: timeline.start.at, legs: timeline.legs } : straightMoves(steps)), [timeline, steps]);
  const tracker = useMemo(() => (timeline ? stateTracker(timeline) : null), [timeline]);
  const clock = useMemo(() => new Clock(route.total_time), [route]);
  useEffect(() => () => clock.dispose(), [clock]);

  // The step under way: the one of the time (a flight or a boat still the step that takes it),
  // within the card chosen while the time is in it (steps of no time at its start belong to
  // it, not to the card before).
  const pinned = useRef<number | null>(null);
  const travels = useMemo(() => travelEnds(steps, moves.legs), [steps, moves]);
  const stepOf = useCallback(
    (time: number) => {
      const i = stepAt(steps, time);
      const p = pinned.current;
      if (p != null && cards[p] && time >= cards[p].start && time <= cards[p].end) return Math.min(Math.max(i, cards[p].first), cards[p].last);
      return i > 0 && (travels.get(i - 1) ?? -1) > time ? i - 1 : i;
    },
    [steps, cards, travels],
  );
  const step = useClock(
    clock,
    useCallback((c: Clock) => stepOf(c.time), [stepOf]),
  );
  const card = cardOf(cards, step);
  const seg = Math.max(
    0,
    segs.findIndex((s) => s.first <= step && step <= s.last),
  );
  const zone = useMemo<[number, number]>(() => (segs[seg] ? [cardOf(cards, segs[seg].first), cardOf(cards, segs[seg].last)] : [0, 0]), [segs, seg, cards]);
  const state = useMemo(() => (tracker ? tracker(step - 1) : null), [tracker, step]);
  const moving = useClock(
    clock,
    useCallback(
      (c: Clock) => {
        const p = positionAt(moves, c.time);
        return p.moving ? moves.legs[p.leg].mode : null;
      },
      [moves],
    ),
  );

  const select = useCallback(
    (n: number) => {
      const c = cards[Math.min(cards.length - 1, Math.max(0, n))];
      if (!c) return;
      pinned.current = c.n;
      clock.seek(c.start);
    },
    [cards, clock],
  );

  const [follow, setFollow] = useState(true);
  const [layers, setLayers] = useState<MapLayers>(() => {
    try {
      return { ...DEFAULT_LAYERS, ...JSON.parse(localStorage.getItem(LAYERS_KEY) ?? "{}") };
    } catch {
      return DEFAULT_LAYERS;
    }
  });
  const changeLayers = (l: MapLayers) => {
    setLayers(l);
    localStorage.setItem(LAYERS_KEY, JSON.stringify(l));
  };

  // The page's height: from the top of the viewer to the bottom of the window (measured again
  // when what is above it changes, as the problem report's form).
  const root = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState(600);
  useLayoutEffect(() => {
    const measure = () => {
      const top = root.current?.getBoundingClientRect().top ?? 0;
      // The content area's padding under it.
      setHeight(Math.max(480, Math.floor(window.innerHeight - top - 21)));
    };
    measure();
    const above = root.current?.parentElement;
    const ro = new ResizeObserver(measure);
    if (above) ro.observe(above);
    window.addEventListener("resize", measure);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, []);

  // Space plays or pauses, the arrows go from card to card (not while typing): the viewer has
  // the keyboard once open.
  useEffect(() => root.current?.focus({ preventScroll: true }), []);
  const onKeyDown = (e: React.KeyboardEvent) => {
    const target = e.target as HTMLElement;
    if (/^(INPUT|SELECT|TEXTAREA)$/.test(target.tagName) || target.getAttribute("role") === "slider") return;
    if (e.key === " ") clock.toggle();
    else if (e.key === "ArrowDown" || e.key === "ArrowRight") select(card + 1);
    else if (e.key === "ArrowUp" || e.key === "ArrowLeft") select(card - 1);
    else return;
    e.preventDefault();
  };

  const checkedCards = useMemo(() => {
    if (!checked) return undefined;
    return new Set(cards.filter((c) => checked.has(c.first)).map((c) => c.n));
  }, [checked, cards]);

  const status = (
    <>
      {timeline ? (
        <Activity clock={clock} stepOf={stepOf} version={version} timeline={timeline} quests={quests} cards={cards} titles={titles} />
      ) : (
        <span className="muted">{failed ? t("Replay indisponible : {error}", { error: failed }) : t("Chargement du replay…")}</span>
      )}
      {onEditSteps && (
        <button className="link gm-edit" onClick={onEditSteps}>
          ✎ {t("Modifier les étapes")}
        </button>
      )}
    </>
  );

  return (
    <div className="guide-viewer" ref={root} style={{ height }} onKeyDown={onKeyDown} tabIndex={-1}>
      <StepList
        steps={steps}
        cards={cards}
        titles={titles}
        segments={segs}
        timeline={timeline}
        quests={quests}
        current={card}
        onSelect={select}
        checked={checkedCards}
        onCheck={onCheck && ((n) => onCheck(Array.from({ length: cards[n].last - cards[n].first + 1 }, (_, k) => cards[n].first + k)))}
      />
      <GuideMap
        steps={steps}
        moves={moves}
        cards={cards}
        titles={titles}
        current={card}
        zone={zone}
        clock={clock}
        follow={follow}
        onFollow={setFollow}
        layers={layers}
        onLayers={changeLayers}
        onSelect={select}
        status={status}
      />
      <CharacterPanel
        route={route}
        request={version.request}
        timeline={timeline}
        quests={quests}
        state={state}
        clock={clock}
        step={step}
        card={card}
        cards={cards.length}
        moving={moving}
      />
      <PlaybackBar clock={clock} segments={segs} current={seg} onStep={(d) => select(card + d)} />
    </div>
  );
}

/** What the character is doing now, over the map ("Killing Young Wolf"). */
function Activity({
  clock,
  stepOf,
  version,
  timeline,
  quests,
  cards,
  titles,
}: {
  clock: Clock;
  stepOf: (time: number) => number;
  version: GuideVersion;
  timeline: Timeline;
  quests: Map<number, QuestInfo>;
  cards: Card[];
  titles: string[];
}) {
  const t = useT();
  const moves = useMemo(() => ({ start: timeline.start.at, legs: timeline.legs }), [timeline]);
  const text = useClock(
    clock,
    useCallback(
      (c: Clock) => {
        const p = positionAt(moves, c.time);
        const i = stepOf(c.time);
        const a = activityAt(version.outcome.route, timeline, quests, i, titles[cardOf(cards, i)], c.time, p.leg, p.moving);
        return `${a.verb}\u0000${a.what}`;
      },
      [moves, version, timeline, quests, stepOf, cards, titles],
    ),
  );
  const [verb, what] = text.split("\u0000");
  return (
    <span className="gm-activity">
      <b>{t(verb)}</b> {what}
    </span>
  );
}
