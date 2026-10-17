import { memo, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { Difficulty, Leg, MapInfo, RouteStep, Step, Suggestions, WorldPoint } from "../api";
import { useAreaName, useT } from "../i18n";
import { type Card, type Moves, positionAt } from "./guideTimeline";
import type { Clock } from "./playback";
import { labelPoint, loadImage, loadIndex, loadMinimapImage, loadMinimapIndex } from "./WorldMap";

/** What the map draws: the kinds of moves, the step markers, the whole guide or the current zone only. */
export interface MapLayers {
  walk: boolean;
  flight: boolean;
  link: boolean;
  markers: boolean;
  scope: "all" | "zone";
}

export const DEFAULT_LAYERS: MapLayers = { walk: true, flight: true, link: true, markers: true, scope: "zone" };

interface Props {
  steps: Step[];
  /** The character's moves (the timeline's, or straight lines between the steps while it loads). */
  moves: Moves;
  cards: Card[];
  titles: string[];
  /** Card under way, when its moves end (a flight or a boat taken at its end goes on after
   * it), and the cards of the zone under way (first, last). */
  current: number;
  currentEnd: number;
  zone: [number, number];
  clock: Clock;
  follow: boolean;
  onFollow: (follow: boolean) => void;
  layers: MapLayers;
  onLayers: (layers: MapLayers) => void;
  onSelect: (card: number) => void;
  /** The line over the map (what the character is doing). */
  status?: ReactNode;
  /** Editing: what could be added where the guide is watched (quests not taken, objectives and
   * turn-ins forgotten), and adding one of them there. */
  suggestions?: Suggestions | null;
  onPick?: (step: RouteStep) => void;
}

/** A place of a suggestion on the map: its kind, what it says, the step it adds. */
interface Offer {
  key: string;
  x: number;
  y: number;
  kind: "untaken" | "objective" | "turn-in";
  glyph: string;
  label: string;
  step: RouteStep;
  /** A quest's color for the character then (its level against the character's). */
  difficulty?: Difficulty;
  /** Done in a dungeon: drawn as a dungeon (a diamond). */
  dungeon?: boolean;
}

/** Continents (game maps), west to east. */
const CONTINENTS = [1, 0, 530];
/** Zoom range, in screen pixels per pixel of the continent's map, relative to the whole continent in view. */
const MAX_ZOOM = 24;
const MIN_ZOOM = 0.6;
/** From this zoom (times the whole continent's) the zones' own maps, sharper, are drawn over the continent's. */
const ZONE_DETAIL = 2;
/** From this zoom (times the whole continent's) the game's minimap, far more detailed, is drawn instead of the zones' maps. */
const MINIMAP_DETAIL = 3;
/** Pixels a side of a minimap image, the levels it has (level l: 2^l game tiles a side), and a game tile's size in yards. */
const MINIMAP_SIZE = 256;
const MINIMAP_LEVELS = 3;
const TILE = 1600 / 3;
/** The minimap's open sea. */
const MINIMAP_SEA = "#05080a";
/** How far past their outlines the zones' maps fill the gaps between them (pixels of the continent's map, both sides). */
const GROW = 8;
/** A zone's map this much over a bigger zone's is a city's (seen from inside, or drawn by that zone's map too). */
const CITY_SHARE = 0.5;
/** Cards before and after the current one with a full marker (the others: a dot). */
const NEAR_BEFORE = 2;
const NEAR_AFTER = 8;

interface View {
  k: number;
  x: number;
  y: number;
}

/** Projects world coordinates (x north, y west) on a map image of `info`. */
function projector(info: MapInfo) {
  const b = info.bounds!;
  return (x: number, y: number): [number, number] => [((b.max_y - y) / (b.max_y - b.min_y)) * info.width, ((b.max_x - x) / (b.max_x - b.min_x)) * info.height];
}

const f1 = (v: number) => Math.round(v * 10) / 10;

/** The moves of `legs` on a continent as path data by kind, in its map's pixels. */
function trace(legs: Leg[], continent: number, project: (x: number, y: number) => [number, number]) {
  const out = { walk: "", flight: "", link: "", jump: "" };
  for (const l of legs) {
    if (l.from[0] !== continent) continue;
    const [x0, y0] = project(l.from[1], l.from[2]);
    if (l.to[0] !== continent) continue;
    const pts = [...(l.via ?? []).map(([x, y]) => project(x, y)), project(l.to[1], l.to[2])];
    const d = `M${f1(x0)},${f1(y0)}${pts.map(([x, y]) => `L${f1(x)},${f1(y)}`).join("")}`;
    if (l.mode === "walk" || l.mode === "mount") out.walk += d;
    else if (l.mode === "flight") out.flight += d;
    else if (l.mode === "link") out.link += d;
    else out.jump += d;
  }
  return out;
}

/** The kind of marker of a card: what most matters among its steps. */
function markerKind(steps: Step[], card: Card): { kind: string; glyph: string } {
  const kinds = new Set(steps.slice(card.first, card.last + 1).map((s) => s.kind));
  if (card.travel) return { kind: "travel", glyph: "➜" };
  if (kinds.has("turnin")) return { kind: "turnin", glyph: "?" };
  if (kinds.has("accept")) return { kind: "accept", glyph: "!" };
  if (kinds.has("dungeon")) return { kind: "dungeon", glyph: "◆" };
  if (kinds.has("objective")) {
    const s = steps.slice(card.first, card.last + 1).find((x) => x.kind === "objective");
    return s?.target?.kind === "npc" ? { kind: "kill", glyph: "✕" } : { kind: "collect", glyph: "◆" };
  }
  if (kinds.has("grind")) return { kind: "grind", glyph: "✕" };
  return { kind: "other", glyph: "•" };
}

/** The guide on the game's map of a continent, big: the route, the steps, the character moving along it. */
export function GuideMap({ steps, moves, cards, titles, current, currentEnd, zone, clock, follow, onFollow, layers, onLayers, onSelect, status, suggestions, onPick }: Props) {
  const t = useT();
  const areaName = useAreaName();
  const [index, setIndex] = useState<MapInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    loadIndex()
      .then(setIndex)
      .catch((e) => setError(String(e)));
  }, []);

  // The game version's continents, and the one shown.
  const continents = useMemo(() => CONTINENTS.filter((c) => index?.some((m) => m.kind === 2 && m.bounds?.map === c)), [index]);
  const [continent, setContinent] = useState<number>(() => moves.start[0]);
  const base = useMemo(
    () => index?.filter((m) => m.kind === 2 && m.bounds?.map === continent).sort((a, b) => b.width - a.width)[0] ?? null,
    [index, continent],
  );
  const project = useMemo(() => (base ? projector(base) : null), [base]);
  // The zones' maps, and where a city is (a map mostly over a bigger zone's: Stormwind,
  // Undercity), covered with the map of the zone around it instead: the zone's map draws the
  // city too, without the seams of two maps drawn differently.
  const [zones, covers] = useMemo(() => {
    if (!base || !project) return [[], []];
    const all = (index ?? [])
      .filter((m) => m.kind === 3 && m.parent === base.id && m.bounds)
      .map((z) => {
        const b = z.bounds!;
        const [x1, y1] = project(b.max_x, b.max_y);
        const [x2, y2] = project(b.min_x, b.min_y);
        const rect = { x: Math.min(x1, x2), y: Math.min(y1, y2), w: Math.abs(x2 - x1), h: Math.abs(y2 - y1) };
        const rings = z.shape?.length ? z.shape : null;
        const d = rings ? rings.map((r) => `M${r.map(([x, y]) => `${x},${y}`).join("L")}Z`).join("") : null;
        return { z, rect, d, label: rings ? labelPoint(rings) : ([rect.x + rect.w / 2, rect.y + rect.h / 2] as [number, number]) };
      });
    type Zone = (typeof all)[number];
    const within = (a: Zone, b: Zone) => {
      const w = Math.min(a.rect.x + a.rect.w, b.rect.x + b.rect.w) - Math.max(a.rect.x, b.rect.x);
      const h = Math.min(a.rect.y + a.rect.h, b.rect.y + b.rect.h) - Math.max(a.rect.y, b.rect.y);
      return b !== a && b.rect.w * b.rect.h > a.rect.w * a.rect.h && w > 0 && h > 0 && w * h >= CITY_SHARE * a.rect.w * a.rect.h;
    };
    const cities = all.flatMap((a) => {
      const around = all.find((b) => within(a, b));
      return around && a.d ? [{ id: a.z.id, area: a.z.area, rect: a.rect, d: a.d, around }] : [];
    });
    return [all.filter((a) => !all.some((b) => within(a, b))), cities];
  }, [index, base, project]);

  // The frame's size, and the view: screen = map * k + (x, y). Kept out of React (it changes at
  // every frame while following); `scale` follows its zoom by steps, for what is drawn at a
  // size of its own (markers, labels, zone maps).
  const frame = useRef<HTMLDivElement>(null);
  const layer = useRef<SVGGElement>(null);
  const dot = useRef<SVGGElement>(null);
  const view = useRef<View>({ k: 1, x: 0, y: 0 });
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [scale, setScale] = useState(1);
  const [shownZones, setShownZones] = useState<number[]>([]);
  const [minimap, setMinimap] = useState<Set<string> | null>(null);
  const [shownTiles, setShownTiles] = useState<MinimapTile[]>([]);
  useEffect(() => {
    loadMinimapIndex()
      .then(setMinimap)
      .catch(() => setMinimap(new Set()));
  }, []);
  const fitK = base && size.w ? Math.min(size.w / base.width, size.h / base.height) : 1;

  useEffect(() => {
    const el = frame.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // The zones in view (their maps drawn when zoomed in), worked out now and then while the view
  // moves (at every frame while following).
  const settle = useRef(0);
  const refresh = useRef(() => {});
  refresh.current = () => {
    settle.current = 0;
    const v = view.current;
    setScale(2 ** (Math.round(Math.log2(v.k) * 4) / 4));
    const [x0, y0, x1, y1] = [-v.x / v.k, -v.y / v.k, (size.w - v.x) / v.k, (size.h - v.y) / v.k];
    const tiles = size.w && base && minimap?.size && v.k >= fitK * MINIMAP_DETAIL ? minimapTiles(base, minimap, v.k, [x0, y0, x1, y1]) : [];
    setShownTiles((old) => (old.map((o) => o.key).join() === tiles.map((o) => o.key).join() ? old : tiles));
    if (!size.w || v.k < fitK * ZONE_DETAIL || tiles.length) return setShownZones((z) => (z.length ? [] : z));
    const ids = zones.filter(({ rect: r }) => r.x < x1 && r.x + r.w > x0 && r.y < y1 && r.y + r.h > y0).map((z) => z.z.id);
    setShownZones((z) => (z.join() === ids.join() ? z : ids));
  };
  const apply = useCallback((v: View) => {
    view.current = v;
    layer.current?.setAttribute("transform", `translate(${v.x} ${v.y}) scale(${v.k})`);
    if (!settle.current) settle.current = window.setTimeout(() => refresh.current(), 150);
  }, []);
  useEffect(() => () => clearTimeout(settle.current), []);
  // The minimap known: drawn if zoomed in already.
  useEffect(() => refresh.current(), [minimap]);

  const center = useCallback(
    (p: WorldPoint, k = view.current.k) => {
      if (!project || !size.w) return;
      const [px, py] = project(p[1], p[2]);
      apply({ k, x: size.w / 2 - px * k, y: size.h / 2 - py * k });
    },
    [project, size, apply],
  );
  const fit = useCallback(() => {
    if (!base || !size.w) return;
    apply({ k: fitK, x: (size.w - base.width * fitK) / 2, y: (size.h - base.height * fitK) / 2 });
  }, [base, size, fitK, apply]);

  // A new continent: the zone under way in view around the character (the whole continent when
  // not following).
  const placed = useRef<string>("");
  useEffect(() => {
    if (!base || !project || !size.w) return;
    const key = `${base.id}`;
    if (placed.current === key) return void refresh.current();
    placed.current = key;
    if (!follow) return fit();
    const area = steps[cards[current]?.first]?.zone_id;
    const rect = zones.find((z) => z.z.area === area)?.rect ?? covers.find((c) => c.area === area)?.rect;
    const k = rect ? Math.min(size.w / rect.w, size.h / rect.h) * 0.95 : fitK * 6;
    center(positionAt(moves, clock.time).at, Math.min(fitK * MAX_ZOOM, k));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [base, project, size, zones, covers]);

  // At each moment: the character's dot, and the view following it.
  const cache = useRef(new Map());
  useEffect(() => {
    cache.current = new Map();
  }, [moves]);
  const draw = useCallback(() => {
    if (!project) return;
    const { at } = positionAt(moves, clock.time, cache.current);
    if (at[0] !== continent) {
      if (follow) setContinent(at[0]);
      dot.current?.setAttribute("visibility", "hidden");
      return;
    }
    const [px, py] = project(at[1], at[2]);
    if (follow && size.w) {
      const v = view.current;
      apply({ k: v.k, x: size.w / 2 - px * v.k, y: size.h / 2 - py * v.k });
    }
    dot.current?.setAttribute("visibility", "visible");
    dot.current?.setAttribute("transform", `translate(${px} ${py}) scale(${1 / view.current.k})`);
  }, [project, moves, clock, continent, follow, size, apply]);
  useEffect(() => {
    draw();
    return clock.subscribe(draw);
  }, [clock, draw]);

  // Wheel zooms around the pointer (the map is the page: no Ctrl needed).
  useEffect(() => {
    const el = frame.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = el.getBoundingClientRect();
      const v = view.current;
      const k = Math.min(fitK * MAX_ZOOM, Math.max(fitK * MIN_ZOOM, v.k * Math.exp(-e.deltaY * 0.0015)));
      // Following: the zoom keeps the character in the middle.
      const [cx, cy] = follow ? [size.w / 2, size.h / 2] : [e.clientX - rect.left, e.clientY - rect.top];
      apply({ k, x: cx - ((cx - v.x) * k) / v.k, y: cy - ((cy - v.y) * k) / v.k });
      draw();
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [fitK, follow, size, apply, draw]);
  const zoomBy = (f: number) => {
    const v = view.current;
    const k = Math.min(fitK * MAX_ZOOM, Math.max(fitK * MIN_ZOOM, v.k * f));
    const [cx, cy] = [size.w / 2, size.h / 2];
    apply({ k, x: cx - ((cx - v.x) * k) / v.k, y: cy - ((cy - v.y) * k) / v.k });
    draw();
  };

  // Dragging moves the map (and stops following the character).
  const drag = useRef<{ x: number; y: number; vx: number; vy: number; moved: boolean } | null>(null);
  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    // A text selected elsewhere would stay greyed (and grow) while the map is dragged.
    window.getSelection()?.removeAllRanges();
    drag.current = { x: e.clientX, y: e.clientY, vx: view.current.x, vy: view.current.y, moved: false };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d || !(e.buttons & 1)) return;
    const [dx, dy] = [e.clientX - d.x, e.clientY - d.y];
    if (!d.moved && Math.abs(dx) + Math.abs(dy) < 4) return;
    if (!d.moved) {
      d.moved = true;
      frame.current?.setPointerCapture(e.pointerId);
      if (follow) onFollow(false);
    }
    apply({ k: view.current.k, x: d.vx + dx, y: d.vy + dy });
  };
  const onPointerUp = () => {
    // The click that ends a drag selects nothing.
    setTimeout(() => (drag.current = null), 0);
  };

  // The route: the whole guide faint, the zone under way, the card under way bright.
  const all = useMemo(() => (project ? trace(moves.legs, continent, project) : null), [moves, continent, project]);
  const span = (from: number, to: number) => moves.legs.filter((l) => l.end > from && l.start < to);
  const zoneTrace = useMemo(
    () => (project && cards[zone[0]] ? trace(span(cards[zone[0]].start, cards[zone[1]].end), continent, project) : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [moves, continent, project, cards, zone[0], zone[1]],
  );
  const lit = useMemo(
    () => (project && cards[current] ? trace(span(cards[current].start, currentEnd), continent, project) : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [moves, continent, project, cards, current, currentEnd],
  );

  // Markers: the cards of the zone under way, those around the current one in full (the next
  // ones first), the others as dots.
  const [hover, setHover] = useState<number | null>(null);
  const markers = useMemo(() => {
    if (!project || !layers.markers) return [];
    const out: { n: number; x: number; y: number; kind: string; glyph: string; near: boolean }[] = [];
    for (let n = zone[0]; n <= zone[1] && n < cards.length; n++) {
      const c = cards[n];
      if (!c.world || c.world[0] !== continent) continue;
      const [x, y] = project(c.world[1], c.world[2]);
      out.push({ n, x, y, ...markerKind(steps, c), near: n >= current - NEAR_BEFORE && n <= current + NEAR_AFTER });
    }
    // The current card on top, the near ones over the dots.
    return out.sort((a, b) => Number(a.near) - Number(b.near) || Number(a.n === current) - Number(b.n === current));
  }, [project, layers.markers, zone, cards, continent, steps, current]);

  // Editing: the quests not taken ("!"), the objectives forgotten (red) and the turn-ins
  // forgotten ("?" in red) on this continent.
  const offers = useMemo(() => {
    if (!project || !suggestions || !onPick) return [];
    const out: Offer[] = [];
    const on = (p: WorldPoint) => p[0] === continent;
    for (const q of suggestions.untaken)
      q.places.filter((p) => on(p.at)).forEach((p, k) => {
        const [x, y] = project(p.at[1], p.at[2]);
        const label = [`[${q.level}] ${q.name}`, p.name, q.dungeon].filter(Boolean).join(" · ");
        out.push({ key: `u${q.quest}/${k}`, x, y, kind: "untaken", glyph: "!", label, step: { kind: "accept", quest: q.quest }, difficulty: q.difficulty, dungeon: !!q.dungeon });
      });
    for (const o of suggestions.objectives)
      o.places.forEach((p, spot) => {
        if (!on(p.at)) return;
        const [x, y] = project(p.at[1], p.at[2]);
        // A dungeon's objective is done in its run: picking it adds the run.
        const step: RouteStep = o.dungeon ? { kind: "dungeon", dungeon: o.dungeon[0] } : { kind: "objective", quest: o.quest, objective: o.objective, spot };
        // One mark per dungeon (its entrance), naming all its objectives left.
        const key = o.dungeon ? `d${o.dungeon[0]}` : `o${o.quest}/${o.objective}/${spot}`;
        const same = o.dungeon && out.find((m) => m.key === key);
        if (same) same.label += ` ; ${o.text}`;
        else out.push({ key, x, y, kind: "objective", glyph: "✕", label: o.dungeon ? `${o.dungeon[1]} : ${o.text}` : `${o.text} · ${o.name}`, step, dungeon: !!o.dungeon });
      });
    for (const q of suggestions.turn_ins)
      q.places.filter((p) => on(p.at)).forEach((p, k) => {
        const [x, y] = project(p.at[1], p.at[2]);
        out.push({ key: `t${q.quest}/${k}`, x, y, kind: "turn-in", glyph: "?", label: `${q.name} · ${p.name}`, step: { kind: "turn_in", quest: q.quest } });
      });
    return out;
  }, [project, suggestions, onPick, continent]);
  const [offered, setOffered] = useState<string | null>(null);

  const routeLayer = (d: { walk: string; flight: string; link: string; jump: string } | null, cls: string) =>
    d && (
      <g className={cls}>
        {layers.walk && d.walk && <path d={d.walk} className="gm-walk" vectorEffect="non-scaling-stroke" />}
        {layers.flight && d.flight && <path d={d.flight} className="gm-flight" vectorEffect="non-scaling-stroke" />}
        {layers.link && d.link && <path d={d.link} className="gm-link" vectorEffect="non-scaling-stroke" />}
        {d.jump && <path d={d.jump} className="gm-jump" vectorEffect="non-scaling-stroke" />}
      </g>
    );
  const s = 1 / scale;
  const shown = useMemo(() => zones.filter((z) => shownZones.includes(z.z.id)), [zones, shownZones]);
  const hidden = useMemo(() => covers.filter((c) => shownZones.includes(c.around.z.id)), [covers, shownZones]);

  return (
    <div className="guide-map">
      <div className="gm-bar">
        <strong>{t("Carte")}</strong>
        {continents.length > 1 && (
          <select
            value={continent}
            onChange={(e) => {
              placed.current = "";
              onFollow(false);
              setContinent(Number(e.target.value));
            }}
            aria-label={t("Continent")}
          >
            {continents.map((c) => {
              const info = index?.filter((m) => m.kind === 2 && m.bounds?.map === c).sort((a, b) => b.width - a.width)[0];
              return (
                <option key={c} value={c}>
                  {info ? areaName(info) : c}
                </option>
              );
            })}
          </select>
        )}
        <button className={`gm-toggle${follow ? " on" : ""}`} aria-pressed={follow} onClick={() => onFollow(!follow)} title={t("La carte suit le personnage")}>
          <span aria-hidden>◎</span> {t("Suivre le personnage")}
        </button>
        <LayersMenu layers={layers} onLayers={onLayers} />
        <span className="gm-bar-fill" />
      </div>
      {status && <div className="gm-status">{status}</div>}
      <div
        className="gm-frame"
        ref={frame}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        onDoubleClick={() => {
          onFollow(false);
          fit();
        }}
      >
        {error && <div className="error">{t("Carte indisponible : {error}", { error })}</div>}
        <svg width={size.w} height={size.h} role="img" aria-label={t("Carte du guide")}>
          <defs>
            {shown.map(({ z, d }) => d && (
              <clipPath key={z.id} id={`gm-clip-${z.id}`}>
                <path d={d} fillRule="evenodd" />
              </clipPath>
            ))}
            {shown.map(({ z, d }) => d && (
              <mask key={z.id} id={`gm-grow-${z.id}`} maskUnits="userSpaceOnUse">
                <path d={d} fillRule="evenodd" fill="#fff" stroke="#fff" strokeWidth={GROW} strokeLinejoin="round" />
              </mask>
            ))}
            {hidden.map((c) => (
              <clipPath key={c.id} id={`gm-cover-${c.id}`}>
                <path d={c.d} fillRule="evenodd" />
              </clipPath>
            ))}
          </defs>
          <g ref={layer}>
            {base && <MapImage id={base.id} x={0} y={0} w={base.width} h={base.height} />}
            {/* The zones' maps a little past their outlines first (biggest under), filling the
                gaps the simplified outlines leave, then each within its outline. */}
            {[...shown]
              .sort((a, b) => b.rect.w * b.rect.h - a.rect.w * a.rect.h)
              .map(({ z, rect, d }) => (
                <g key={z.id} mask={d ? `url(#gm-grow-${z.id})` : undefined}>
                  <MapImage id={z.id} x={rect.x} y={rect.y} w={rect.w} h={rect.h} />
                </g>
              ))}
            {shown.map(({ z, rect, d }) => (
              <g key={z.id} clipPath={d ? `url(#gm-clip-${z.id})` : undefined}>
                <MapImage id={z.id} x={rect.x} y={rect.y} w={rect.w} h={rect.h} />
              </g>
            ))}
            {hidden.map(({ id, around: { z, rect } }) => (
              <g key={id} clipPath={`url(#gm-cover-${id})`}>
                <MapImage id={z.id} x={rect.x} y={rect.y} w={rect.w} h={rect.h} />
              </g>
            ))}
            {/* The open sea, where the game has no minimap: its color there, not the continent's parchment. */}
            {shownTiles.length > 0 && base && <rect x={-base.width} y={-base.height} width={3 * base.width} height={3 * base.height} fill={MINIMAP_SEA} />}
            {shownTiles.map((tile) => (
              <MinimapImage key={tile.key} tile={tile} />
            ))}
            {scale >= 1.2 * fitK &&
              zones
                .filter((z) => z.z.area)
                .map(({ z, label }) => (
                  <text key={z.id} className="gm-zone-label" transform={`translate(${label[0]} ${label[1]}) scale(${s})`} textAnchor="middle">
                    {areaName(z)}
                  </text>
                ))}
            {layers.scope === "all" && routeLayer(all, "gm-all")}
            {routeLayer(zoneTrace, layers.scope === "all" ? "gm-zone" : "gm-zone only")}
            {routeLayer(lit, "gm-lit")}
            {markers.map((m) => (
              <g
                key={m.n}
                className={`gm-marker gm-${m.kind}${m.n === current ? " current" : ""}${m.near ? "" : " far"}`}
                transform={`translate(${m.x} ${m.y}) scale(${s})`}
                onClick={() => !drag.current?.moved && onSelect(m.n)}
                onPointerEnter={() => setHover(m.n)}
                onPointerLeave={() => setHover((h) => (h === m.n ? null : h))}
              >
                <circle r={m.n === current ? 11 : m.near ? 8 : 4} />
                {m.near && (
                  <text textAnchor="middle" dominantBaseline="central">
                    {m.glyph}
                  </text>
                )}
                {(m.n === current || m.n === hover) && (
                  <g className="gm-label" transform="translate(16 0)">
                    <text dominantBaseline="central">
                      {m.n + 1}. {titles[m.n]}
                    </text>
                  </g>
                )}
              </g>
            ))}
            {offers.map((o) => (
              <g
                key={o.key}
                className={`gm-offer gm-${o.kind}${o.difficulty ? ` gm-diff-${o.difficulty}` : ""}${o.dungeon ? " gm-in-dungeon" : ""}`}
                transform={`translate(${o.x} ${o.y}) scale(${s})`}
                onClick={() => !drag.current?.moved && onPick?.(o.step)}
                onPointerEnter={() => setOffered(o.key)}
                onPointerLeave={() => setOffered((k) => (k === o.key ? null : k))}
              >
                {o.dungeon ? <rect className="gm-offer-shape" x={-8} y={-8} width={16} height={16} transform="rotate(45)" /> : <circle className="gm-offer-shape" r={9} />}
                <text textAnchor="middle" dominantBaseline="central">
                  {o.glyph}
                </text>
                {offered === o.key && (
                  <g className="gm-label" transform="translate(16 0)">
                    <text dominantBaseline="central">{o.label}</text>
                  </g>
                )}
              </g>
            ))}
            <g ref={dot} className="gm-dot" visibility="hidden">
              <circle r={14} className="gm-dot-halo" />
              <circle r={6.5} className="gm-dot-core" />
            </g>
          </g>
        </svg>
        <div className="gm-legend" aria-hidden>
          <span>
            <i className="gm-walk" /> {t("À pied")}
          </span>
          <span>
            <i className="gm-flight" /> {t("Vol")}
          </span>
          <span>
            <i className="gm-link" /> {t("Bateau")}
          </span>
        </div>
        <div className="gm-zoom">
          <button onClick={() => zoomBy(1.6)} aria-label={t("Zoomer")} title={t("Zoomer")}>
            +
          </button>
          <button onClick={() => zoomBy(1 / 1.6)} aria-label={t("Dézoomer")} title={t("Dézoomer")}>
            −
          </button>
          <button
            onClick={() => {
              onFollow(false);
              fit();
            }}
            aria-label={t("Tout le continent")}
            title={t("Tout le continent")}
          >
            ⤢
          </button>
        </div>
      </div>
    </div>
  );
}

/** A map image of the game, loaded once. */
/** A minimap image in view: its game map, level and place, and its rectangle on the continent's map. */
interface MinimapTile {
  key: string;
  map: number;
  level: number;
  x: number;
  y: number;
  rect: { x: number; y: number; w: number; h: number };
}

/** The minimap images over `box` (x0, y0, x1, y1 on `base`'s map) at zoom `k`: of the level
 * whose images are drawn closest to their own size. */
function minimapTiles(base: MapInfo, have: Set<string>, k: number, [x0, y0, x1, y1]: number[]): MinimapTile[] {
  const b = base.bounds!;
  const map = b.map;
  // A game tile (x eastwards from world y = 32 tiles, y southwards from world x = 32 tiles) on the map.
  const sx = (base.width / (b.max_y - b.min_y)) * TILE;
  const sy = (base.height / (b.max_x - b.min_x)) * TILE;
  const ox = ((b.max_y - 32 * TILE) / (b.max_y - b.min_y)) * base.width;
  const oy = ((b.max_x - 32 * TILE) / (b.max_x - b.min_x)) * base.height;
  const level = Math.max(0, Math.min(MINIMAP_LEVELS - 1, Math.round(Math.log2(MINIMAP_SIZE / (sx * k)))));
  const span = 2 ** level;
  const [w, h] = [sx * span, sy * span];
  const out: MinimapTile[] = [];
  for (let gx = Math.max(0, Math.floor((x0 - ox) / w)); gx <= Math.min(64 / span - 1, Math.floor((x1 - ox) / w)); gx++) {
    for (let gy = Math.max(0, Math.floor((y0 - oy) / h)); gy <= Math.min(64 / span - 1, Math.floor((y1 - oy) / h)); gy++) {
      const key = `${map}/${level}/${gx}/${gy}`;
      if (have.has(key)) out.push({ key, map, level, x: gx, y: gy, rect: { x: ox + gx * w, y: oy + gy * h, w, h } });
    }
  }
  return out;
}

const MinimapImage = memo(function MinimapImage({ tile: { map, level, x, y, rect } }: { tile: MinimapTile }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    loadMinimapImage(map, level, x, y)
      .then((s) => live && setSrc(s))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [map, level, x, y]);
  // A hair bigger: no seam between neighbors.
  const pad = rect.w / 512;
  return src ? <image href={src} x={rect.x - pad} y={rect.y - pad} width={rect.w + 2 * pad} height={rect.h + 2 * pad} preserveAspectRatio="none" className="gm-image" /> : null;
});

const MapImage = memo(function MapImage({ id, x, y, w, h }: { id: number; x: number; y: number; w: number; h: number }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    loadImage(id)
      .then((s) => live && setSrc(s))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [id]);
  return src ? <image href={src} x={x} y={y} width={w} height={h} preserveAspectRatio="none" className="gm-image" /> : null;
});

/** What the map draws, in a menu. */
function LayersMenu({ layers, onLayers }: { layers: MapLayers; onLayers: (l: MapLayers) => void }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const menu = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => !menu.current?.contains(e.target as Node) && setOpen(false);
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [open]);
  const check = (key: "walk" | "flight" | "link" | "markers", label: string) => (
    <label>
      <input type="checkbox" checked={layers[key]} onChange={(e) => onLayers({ ...layers, [key]: e.target.checked })} /> {label}
    </label>
  );
  return (
    <div className="gm-layers" ref={menu}>
      <button aria-expanded={open} onClick={() => setOpen(!open)}>
        {t("Calques")} ▾
      </button>
      {open && (
        <div className="gm-menu" role="group" aria-label={t("Calques")}>
          {check("walk", t("Trajets à pied"))}
          {check("flight", t("Vols"))}
          {check("link", t("Bateaux et zeppelins"))}
          {check("markers", t("Étapes sur la carte"))}
          <hr />
          <label>
            <input type="radio" checked={layers.scope === "all"} onChange={() => onLayers({ ...layers, scope: "all" })} /> {t("Tout le guide")}
          </label>
          <label>
            <input type="radio" checked={layers.scope === "zone"} onChange={() => onLayers({ ...layers, scope: "zone" })} /> {t("La zone en cours")}
          </label>
        </div>
      )}
    </div>
  );
}
