import { useEffect, useMemo, useRef, useState } from "react";
import { api, type MapInfo } from "../api";
import { useAreaName, useT } from "../i18n";

export type WorldPoint = [number, number, number];
export type ZoneState = "preferred" | "forced" | "excluded";

export interface MapMarker {
  at: WorldPoint;
  label: string;
  title?: string;
}

interface Props {
  /** Continents shown side by side (game maps: 1 Kalimdor, 0 Eastern Kingdoms, 530 Outland),
   * west to east; all those of the game version when unset. */
  continents?: number[];
  route?: WorldPoint[];
  markers?: MapMarker[];
  /** Zone states by game zone (AreaTable ID). */
  zoneStates?: Record<number, ZoneState>;
  onZoneClick?: (area: number, name: string) => void;
  /** Part of the route drawn over it (the steps of a zone). */
  highlight?: WorldPoint[];
  /** Animate the route (while it is being optimized). */
  live?: boolean;
  /** Maximum height in pixels. */
  height?: number;
}

let indexPromise: Promise<MapInfo[]> | null = null;
const images = new Map<number, Promise<string>>();
/** Every map of the game version (loaded once). */
export const loadIndex = () => (indexPromise ??= api.mapIndex());
/** A map image (loaded once). */
export const loadImage = (id: number) => {
  let p = images.get(id);
  if (!p) {
    p = api.mapImage(id);
    images.set(id, p);
  }
  return p;
};

/** Forget the maps (another game version shows other maps). */
export function clearMapCache() {
  indexPromise = null;
  for (const p of images.values()) p.then(URL.revokeObjectURL, () => {});
  images.clear();
}

/** Our own map gives each zone one of these earth tones (`.tint-N`), as the game's map does. */
const LAND_TINTS = 6;

/** Continents (game maps), west to east. */
const CONTINENTS = [1, 0, 530];


/** Projects world coordinates (x north, y west) on a map image of `info`. */
function projector(info: MapInfo) {
  const b = info.bounds!;
  return (x: number, y: number): [number, number] => [
    ((b.max_y - y) / (b.max_y - b.min_y)) * info.width,
    ((b.max_x - x) / (b.max_x - b.min_x)) * info.height,
  ];
}

interface Panel {
  info: MapInfo;
  /** Where the map image starts, and the part of it shown (our own map keeps only its land). */
  dx: number;
  width: number;
  project: (x: number, y: number) => [number, number];
}

/** Color of the route at `f` (0 start, 1 end): light blue, blue, violet, magenta. */
const RAMP = [
  [94, 200, 255],
  [57, 135, 229],
  [122, 79, 214],
  [192, 57, 155],
];
function rampColor(f: number): string {
  const x = Math.min(1, Math.max(0, f)) * (RAMP.length - 1);
  const i = Math.min(RAMP.length - 2, Math.floor(x));
  const c = RAMP[i].map((v, k) => Math.round(v + (RAMP[i + 1][k] - v) * (x - i)));
  return `rgb(${c.join(",")})`;
}
const RAMP_CSS = `linear-gradient(90deg, ${[0, 1 / 3, 2 / 3, 1].map(rampColor).join(", ")})`;
/** Route pieces colored along the ramp. */
const CHUNKS = 8;

export function WorldMap({ continents, route, markers, highlight, zoneStates, onZoneClick, live, height = 520 }: Props) {
  const t = useT();
  const areaName = useAreaName();
  const [index, setIndex] = useState<MapInfo[] | null>(null);
  const [loaded, setLoaded] = useState<Record<number, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState({ scale: 1, x: 0, y: 0 });
  const [hover, setHover] = useState<{ id: number; name: string } | null>(null);
  // Choosing zones: our own map, the zones drawn as they are clicked. A route: the game's
  // parchment (darkened), its relief and coasts telling why the route goes where it goes.
  const look = onZoneClick ? "vector" : "parchment";
  const drag = useRef<{ x: number; y: number; vx: number; vy: number; moved: boolean } | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const frameRef = useRef<HTMLDivElement>(null);

  // Ctrl + wheel zooms (the wheel alone scrolls the page). A native, non-passive listener is
  // needed to stop the page scroll and the webview zoom.
  useEffect(() => {
    const frame = frameRef.current;
    if (!frame) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      const rect = frame.getBoundingClientRect();
      const cx = e.clientX - rect.left;
      const cy = e.clientY - rect.top;
      setView((v) => {
        const scale = Math.min(8, Math.max(1, v.scale * (e.deltaY < 0 ? 1.2 : 1 / 1.2)));
        const f = scale / v.scale;
        return { scale, x: scale === 1 ? 0 : cx - (cx - v.x) * f, y: scale === 1 ? 0 : cy - (cy - v.y) * f };
      });
    };
    frame.addEventListener("wheel", onWheel, { passive: false });
    return () => frame.removeEventListener("wheel", onWheel);
  });

  useEffect(() => {
    loadIndex().then(setIndex).catch((e) => setError(String(e)));
  }, []);

  // Continent maps shown, west to east.
  const shown = (continents ?? CONTINENTS).join(",");
  const panels = useMemo<Panel[]>(() => {
    if (!index) return [];
    const base = (map: number) =>
      index.filter((m) => m.kind === 2 && m.bounds?.map === map).sort((a, b) => b.width - a.width)[0];
    const wanted = shown.split(",").map(Number);
    const maps = CONTINENTS.filter((c) => wanted.includes(c)).map(base).filter(Boolean);
    let at = 0;
    return maps.map((info) => {
      // Our own map: each continent from its westmost to its eastmost zone, a sea's width apart.
      const xs = look === "vector" ? index.filter((m) => m.kind === 3 && m.parent === info.id && m.shape?.length).flatMap((m) => m.shape!.flat().map(([x]) => x)) : [];
      const from = xs.length ? Math.min(...xs) - 40 : 0;
      const width = xs.length ? Math.max(...xs) + 40 - from : info.width;
      const p = { info, dx: at - from, width, project: projector(info) };
      at += width;
      return p;
    });
  }, [index, shown, look]);
  const key = panels.map((p) => p.info.id).join("+");

  useEffect(() => {
    setView({ scale: 1, x: 0, y: 0 });
  }, [key]);
  useEffect(() => {
    if (look !== "parchment") return;
    for (const p of panels) {
      loadImage(p.info.id)
        .then((src) => setLoaded((l) => ({ ...l, [p.info.id]: src })))
        .catch((e) => setError(String(e)));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, look]);

  const zones = useMemo(
    () =>
      panels.flatMap((panel) =>
        (index ?? [])
          .filter((m) => m.kind === 3 && m.parent === panel.info.id && m.bounds && m.area)
          .map((z) => {
            const b = z.bounds!;
            const [x1, y1] = panel.project(b.max_x, b.max_y);
            const [x2, y2] = panel.project(b.min_x, b.min_y);
            const rect = { x: panel.dx + Math.min(x1, x2), y: Math.min(y1, y2), w: Math.abs(x2 - x1), h: Math.abs(y2 - y1) };
            // The zone's outline, else its rectangle.
            const rings: [number, number][][] = z.shape?.length
              ? z.shape.map((r) => r.map(([x, y]) => [panel.dx + x, y] as [number, number]))
              : [[[rect.x, rect.y], [rect.x + rect.w, rect.y], [rect.x + rect.w, rect.y + rect.h], [rect.x, rect.y + rect.h]]];
            const d = rings.map((r) => `M${r.map(([x, y]) => `${x},${y}`).join("L")}Z`).join("");
            return { z, rings, d, label: labelPoint(rings), area: rings.reduce((s, r) => s + Math.abs(ringArea(r)), 0) };
          }),
      ),
    [index, panels],
  );

  if (error) return <div className="error">{t("Carte indisponible : {error}", { error })}</div>;
  if (!panels.length) return <div className="map-frame" style={{ aspectRatio: "3 / 2" }} />;

  const W = panels.reduce((s, p) => s + p.width, 0);
  const H = Math.max(...panels.map((p) => p.info.height));
  const s = panels.length > 2 ? 2.2 : panels.length > 1 ? 1.6 : 1; // maps side by side are drawn smaller
  // Our own map is framed on the land (the game's images keep wide seas around it).
  const outlined = look === "vector" ? zones.filter((r) => r.z.shape?.length) : [];
  const land = outlined.map((r) => r.d).join("");
  const box = (() => {
    if (!outlined.length) return { x: 0, y: 0, w: W, h: H };
    const xs = outlined.flatMap((r) => r.rings.flat().map(([x]) => x));
    const ys = outlined.flatMap((r) => r.rings.flat().map(([, y]) => y));
    const pad = 24 * s;
    const x = Math.min(...xs) - pad;
    const y = Math.min(...ys) - pad;
    return { x, y, w: Math.max(...xs) + pad - x, h: Math.max(...ys) + pad - y };
  })();
  const panelOf = (map: number) => panels.find((p) => p.info.bounds!.map === map);
  const project = (p: WorldPoint): [number, number] | null => {
    const panel = panelOf(p[0]);
    if (!panel) return null;
    const [x, y] = panel.project(p[1], p[2]);
    return [panel.dx + x, y];
  };
  // Smallest zone whose outline holds the point (zones without an outline overlap).
  const zoneAt = (px: number, py: number) =>
    zones.filter((r) => r.rings.reduce((inside, ring) => inside !== inRing(ring, px, py), false)).sort((a, b) => a.area - b.area)[0];
  const toMap = (e: React.MouseEvent) => {
    const ctm = svgRef.current?.getScreenCTM();
    if (!ctm) return null;
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(ctm.inverse());
    return [p.x, p.y] as const;
  };

  // Local moves as a solid line colored from start to end; long jumps (flights, hearthstone)
  // as faint dashes; trips between continents (boats, zeppelins) drawn from one map to the other.
  const fmt = (q: [number, number]) => `${q[0].toFixed(1)},${q[1].toFixed(1)}`;
  const trace = (points: WorldPoint[], chunks: number) => {
    const out = { pieces: Array.from({ length: chunks }, () => ""), jumps: "", crossings: "" };
    let prev: WorldPoint | null = null;
    let prevXY: [number, number] | null = null;
    // Piece the pen is drawing in (-1: lifted).
    let pen = -1;
    points.forEach((p, i) => {
      const q = project(p);
      const c = Math.min(chunks - 1, Math.floor((i * chunks) / points.length));
      if (!q) {
        prev = null;
        prevXY = null;
        pen = -1;
        return;
      }
      if (!prev || !prevXY) pen = -1;
      else if (prev[0] !== p[0]) {
        out.crossings += `M${fmt(prevXY)}L${fmt(q)}`;
        pen = -1;
      } else if (Math.hypot(p[1] - prev[1], p[2] - prev[2]) > 1200) {
        out.jumps += `M${fmt(prevXY)}L${fmt(q)}`;
        pen = -1;
      } else if (pen === c) out.pieces[c] += `L${fmt(q)}`;
      else {
        out.pieces[c] += `M${fmt(prevXY)}L${fmt(q)}`;
        pen = c;
      }
      prev = p;
      prevXY = q;
    });
    return out;
  };
  const { pieces, jumps, crossings } = trace(route ?? [], CHUNKS);
  const path = pieces.join("");
  const lit = highlight?.length ? trace(highlight, 1).pieces[0] : "";
  const k = 1 / view.scale; // keep strokes and labels readable when zoomed
  // Markers too close to be read apart are drawn as one: the first level and how many more
  // (all of them on hover). Groups are a marker apart, so they never overlap.
  const groups: { q: [number, number]; labels: string[]; titles: string[] }[] = [];
  for (const m of markers ?? []) {
    const q = project(m.at);
    if (!q) continue;
    const near = groups.find((g) => Math.hypot(g.q[0] - q[0], g.q[1] - q[1]) < 34 * k * s);
    if (near) {
      near.labels.push(m.label);
      near.titles.push(m.title ?? m.label);
    } else groups.push({ q, labels: [m.label], titles: [m.title ?? m.label] });
  }

  return (
    <div className="world-map">
      <div className="map-bar">
        {path && (
          <span className="route-legend" aria-hidden>
            {t("début")} <i style={{ background: RAMP_CSS }} /> {t("fin")}
          </span>
        )}
        <span className="map-hover">{hover?.name ?? (onZoneClick ? t("Cliquez une zone : privilégiée → forcée → exclue → neutre · Ctrl + molette : zoom") : t("Ctrl + molette : zoom · glisser : déplacer · double-clic : vue entière"))}</span>
      </div>
      <div
        className={`map-frame ${look}`}
        // Our own map fills the width at the given height (the sky around it); the game's keeps its proportions.
        style={look === "vector" ? { height } : { aspectRatio: `${box.w} / ${box.h}`, maxWidth: height ? (height * box.w) / box.h : undefined }}
        ref={frameRef}
        onMouseDown={(e) => (drag.current = { x: e.clientX, y: e.clientY, vx: view.x, vy: view.y, moved: false })}
        onMouseMove={(e) => {
          const d = drag.current;
          if (d && e.buttons === 1) {
            const dx = e.clientX - d.x;
            const dy = e.clientY - d.y;
            if (Math.abs(dx) + Math.abs(dy) > 4) d.moved = true;
            if (d.moved) setView((v) => ({ ...v, x: d.vx + dx, y: d.vy + dy }));
          } else {
            const p = toMap(e);
            const hit = p && zoneAt(p[0], p[1]);
            setHover(hit ? { id: hit.z.id, name: areaName(hit.z) } : null);
          }
        }}
        onMouseUp={() => setTimeout(() => (drag.current = null), 0)}
        onMouseLeave={() => {
          drag.current = null;
          setHover(null);
        }}
        onDoubleClick={() => setView({ scale: 1, x: 0, y: 0 })}
      >
        <div className="map-inner" style={{ transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})` }}>
          <svg
            ref={svgRef}
            role="img"
            aria-label={onZoneClick ? t("Carte des zones : cliquez une zone, ou utilisez la recherche à côté") : t("Carte du guide")}
            viewBox={`${box.x} ${box.y} ${box.w} ${box.h}`}
            preserveAspectRatio={look === "vector" ? "xMidYMid meet" : "none"}
            onClick={(e) => {
              if (!onZoneClick || drag.current?.moved) return;
              const p = toMap(e);
              const hit = p && zoneAt(p[0], p[1]);
              if (hit) onZoneClick(hit.z.area!, hit.z.name);
            }}
          >
            <defs>
              <pattern id="hatch" patternUnits="userSpaceOnUse" width={8 * s} height={8 * s} patternTransform="rotate(45)">
                <line x1="0" y1="0" x2="0" y2={8 * s} stroke="rgba(255, 220, 220, 0.75)" strokeWidth={2 * s} />
              </pattern>
            </defs>
            {look === "vector" && (
              <>
                {/* The zones' outlines are simplified and leave gaps: a thick stroke under them
                    joins them into land masses. */}
                <path d={land} className="land-mass" strokeWidth={9 * s} strokeLinejoin="round" />
                {outlined.map((r) => (
                  <path key={r.z.id} d={r.d} className={`zone-land tint-${(r.z.area ?? r.z.id) % LAND_TINTS}`} fillRule="evenodd" strokeWidth={1 * k * s} strokeLinejoin="round" />
                ))}
              </>
            )}
            {look === "parchment" &&
              panels.map((p) =>
              loaded[p.info.id] ? (
                <image key={p.info.id} href={loaded[p.info.id]} x={p.dx} y={0} width={p.info.width} height={p.info.height} preserveAspectRatio="none" />
              ) : (
                <text key={p.info.id} x={p.dx + p.info.width / 2} y={H / 2} textAnchor="middle" className="map-loading-text">
                  {t("Chargement de la carte…")}
                </text>
              ),
              )}
            {onZoneClick && hover && (
              <path d={zones.find((r) => r.z.id === hover.id)?.d} className="zone-hover" fillRule="evenodd" strokeWidth={2.5 * k * s} strokeLinejoin="round" />
            )}
            {zones
              .filter((r) => zoneStates?.[r.z.area!])
              .map((r) => (
                <g key={r.z.id} className={`zone-mark ${zoneStates![r.z.area!]}`}>
                  <path d={r.d} fillRule="evenodd" strokeWidth={1.6 * k * s} strokeLinejoin="round" />
                  {zoneStates![r.z.area!] === "excluded" && <path d={r.d} fillRule="evenodd" className="zone-hatch" />}
                  {/* No names (the list beside the map has them); a dot shows a zone too small to be seen. */}
                  {r.area < 900 && <circle cx={r.label[0]} cy={r.label[1]} r={4 * k * s} />}
                </g>
              ))}
            {jumps && <path d={jumps} className="route-jump" strokeWidth={1.2 * k * s} />}
            {crossings && <path d={crossings} className="route-crossing" strokeWidth={1.2 * k * s} />}
            {path && (
              <g key={live ? path.length : "static"} className={live ? "live" : undefined}>
                <path d={path} className="route-halo" strokeWidth={6 * k * s} />
                {pieces.map((d, i) => d && <path key={i} d={d} className="route-line" stroke={rampColor(i / (CHUNKS - 1))} strokeWidth={2.6 * k * s} />)}
              </g>
            )}
            {lit && (
              <>
                <path d={lit} className="route-halo" strokeWidth={9 * k * s} />
                <path d={lit} className="route-highlight" strokeWidth={4.5 * k * s} />
              </>
            )}
            {groups.map((g, i) => (
              <g key={i} className="map-marker" transform={`translate(${g.q[0]},${g.q[1]}) scale(${k * s})`}>
                <title>{g.titles.join("\n")}</title>
                <circle r={15} />
                <text textAnchor="middle" dominantBaseline="central" fontSize={15}>
                  {g.labels[0]}
                </text>
                {g.labels.length > 1 && (
                  <g className="map-marker-more" transform="translate(13,-13)">
                    <circle r={9} />
                    <text textAnchor="middle" dominantBaseline="central" fontSize={10}>
                      +{g.labels.length - 1}
                    </text>
                  </g>
                )}
              </g>
            ))}
          </svg>
        </div>
      </div>
    </div>
  );
}

/** The big stages of a route: a marker each time it settles in another zone (level shown). */
export function routeMarkers(
  steps: { zone?: string; level: number; time: number; world?: WorldPoint }[],
  title: (zone: string, level: number) => string = (zone, level) => `${zone} · niveau ${level}`,
  minStops = 12,
): MapMarker[] {
  const out: MapMarker[] = [];
  let i = 0;
  while (i < steps.length) {
    const zone = steps[i].zone;
    let j = i;
    while (j < steps.length && (steps[j].zone === zone || !steps[j].zone)) j++;
    const first = steps.slice(i, j).find((s) => s.world);
    if (zone && first?.world && j - i >= minStops && out[out.length - 1]?.title?.split(" · ")[0] !== zone) {
      out.push({ at: first.world, label: String(first.level), title: title(zone, first.level) });
    }
    i = Math.max(j, i + 1);
  }
  return out;
}

/** Ray casting: whether a point is inside a ring. */
function inRing(ring: [number, number][], x: number, y: number): boolean {
  let inside = false;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const [xi, yi] = ring[i];
    const [xj, yj] = ring[j];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

/** Signed area (shoelace). */
function ringArea(ring: [number, number][]): number {
  let a = 0;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) a += ring[j][0] * ring[i][1] - ring[i][0] * ring[j][1];
  return a / 2;
}

/** Where to write a zone's name: the centroid of its largest ring, or the inner point closest to it. */
function labelPoint(rings: [number, number][][]): [number, number] {
  const ring = rings.reduce((a, b) => (Math.abs(ringArea(b)) > Math.abs(ringArea(a)) ? b : a));
  const area = ringArea(ring);
  let cx = 0;
  let cy = 0;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const f = ring[j][0] * ring[i][1] - ring[i][0] * ring[j][1];
    cx += (ring[j][0] + ring[i][0]) * f;
    cy += (ring[j][1] + ring[i][1]) * f;
  }
  const c: [number, number] = area ? [cx / (6 * area), cy / (6 * area)] : ring[0];
  if (inRing(ring, c[0], c[1])) return c;
  // Concave shape: scan the ring's box for the inside point nearest the centroid.
  const xs = ring.map((p) => p[0]);
  const ys = ring.map((p) => p[1]);
  let best: [number, number] = ring[0];
  let bestD = Infinity;
  for (let x = Math.min(...xs); x <= Math.max(...xs); x += 4)
    for (let y = Math.min(...ys); y <= Math.max(...ys); y += 4) {
      const d = (x - c[0]) ** 2 + (y - c[1]) ** 2;
      if (d < bestD && inRing(ring, x, y)) [best, bestD] = [[x, y], d];
    }
  return best;
}
