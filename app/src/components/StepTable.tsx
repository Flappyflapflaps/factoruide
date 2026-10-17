import { Fragment, useMemo, useRef, useState } from "react";
import { fmtTime, type Route, type Step } from "../api";
import { useT } from "../i18n";
import { KIND_LABELS, segments, steps, type Segment } from "./segments";
import { keepTableInView, keepZoneInView, useFollowPointer } from "./zoneTable";
import { useWidth } from "./useWidth";
import { ZoneMap, type NumberedStep } from "./ZoneMap";

/**
 * The guide line by line, read only: a row per zone (or dungeon run) that unfolds into its
 * steps, the zone's map beside the table (the zone under the pointer, else the first one open
 * in view).
 */
export function StepTable({ route }: { route: Route }) {
  const t = useT();
  const [filter, setFilter] = useState("");
  // Zones opened, the one shown on the map, and the step pointed at (number).
  const [open, setOpen] = useState<Set<number>>(new Set());
  const [focus, setFocus] = useState<number | null>(null);
  const [active, setActive] = useState<number | null>(null);
  const [frame, width] = useWidth(1000);
  const table = useRef<HTMLTableElement>(null);
  // Wide enough: the zone map beside the table.
  const wide = width >= 1150;
  const segs = useMemo(() => segments(route.steps), [route]);
  const kindLabel = (s: Step) => t(KIND_LABELS[s.kind] ?? s.kind);
  const numbered = (i: number): NumberedStep => ({ ...route.steps[i], n: i + 1 });
  const stepsOf = (s: Segment) => Array.from({ length: steps(s) }, (_, k) => s.first + k);
  // Filtered: the zones with matching steps, opened on them (all their steps when the zone matches).
  const q = filter.trim().toLowerCase();
  const rows = segs
    .map((s, i) => {
      const all = stepsOf(s);
      if (!q || (s.zone ?? "").toLowerCase().includes(q)) return { s, i, shown: all };
      const x = (k: number) => route.steps[k];
      return { s, i, shown: all.filter((k) => x(k).text.toLowerCase().includes(q) || kindLabel(x(k)).toLowerCase().includes(q)) };
    })
    .filter((r) => !q || r.shown.length > 0);
  const isOpen = (i: number) => (q ? true : open.has(i));
  useFollowPointer(table, wide, "tr.sub-row", setFocus, [open, q]);
  const toggle = (i: number) => {
    const next = new Set(open);
    if (next.delete(i)) {
      if (focus === i) setFocus(null);
    } else {
      next.add(i);
      setFocus(i);
    }
    setOpen(next);
  };
  const duration = (seconds: number) => (seconds >= 30 ? fmtTime(seconds) : "");
  const focused = focus !== null ? segs[focus] : null;

  return (
    <section className="step-table">
      <div className="route-toolbar">
        <input className="search" placeholder={t("Filtrer (quête, zone, type…)")} value={filter} onChange={(e) => setFilter(e.target.value)} />
        <button onClick={() => setOpen(new Set(segs.map((_, i) => i)))} disabled={!!q}>
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
      <div className={`route-layout${wide ? " wide" : ""}`} ref={frame}>
        {/* Fixed columns: opening a zone does not move them. */}
        <table className="table route-table" ref={table}>
          <colgroup>
            <col className="c-time" />
            <col className="c-time" />
            <col className="c-level" />
            <col />
            <col className="c-last" />
          </colgroup>
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
            {rows.map(({ s, i, shown }) => (
              <Fragment key={i}>
                <tr
                  data-seg={i}
                  className={`clickable zone-line${focus === i ? " selected" : ""}`}
                  onClick={(e) => {
                    if (q) return;
                    if (isOpen(i)) keepZoneInView(e.currentTarget);
                    toggle(i);
                  }}
                >
                  <td className="num">{fmtTime(s.start)}</td>
                  <td className="num">{duration(s.end - s.start)}</td>
                  <td>{s.fromLevel === s.toLevel ? s.fromLevel : `${s.fromLevel} → ${s.toLevel}`}</td>
                  <td>
                    {/* Its click reaches the row, which opens or closes. */}
                    <button className="disclosure" aria-expanded={isOpen(i)} aria-label={isOpen(i) ? t("Replier") : t("Déplier")} disabled={!!q}>
                      {isOpen(i) ? "▾" : "▸"}
                    </button>{" "}
                    {s.dungeon && <span className="kind kind-dungeon">{t("Donjon")}</span>} {s.zone ?? <em className="muted">{t("Préparation")}</em>}
                  </td>
                  <td className="num">{steps(s)}</td>
                </tr>
                {isOpen(i) &&
                  shown.map((k) => {
                    const x = route.steps[k];
                    const begin = k > 0 ? route.steps[k - 1].time : 0;
                    return (
                      <tr key={k} data-seg={i} className={`sub-row${active === k + 1 ? " active" : ""}`} onMouseEnter={() => setActive(k + 1)} onMouseLeave={() => setActive(null)}>
                        <td className="num">{fmtTime(begin)}</td>
                        <td className={`num${x.time - begin >= 1800 ? " long" : ""}`}>{duration(x.time - begin)}</td>
                        <td>{x.level}</td>
                        <td>
                          <span className="step-n">{k + 1}</span> <span className={`kind kind-${x.kind}`}>{kindLabel(x)}</span> {x.text}
                          {x.bg && <span className="along"> {t("en chemin")}</span>}
                          {x.farm ? <span className="along"> {t("(tuer ~{n} mobs en chemin)", { n: x.farm })}</span> : null}
                        </td>
                        <td className="num coords">{x.map ? `${x.map.x.toFixed(1)}, ${x.map.y.toFixed(1)}` : ""}</td>
                      </tr>
                    );
                  })}
              </Fragment>
            ))}
          </tbody>
        </table>
        {wide && (
          <aside className="zone-panel">
            {focused ? (
              <ZoneMap title={focused.zone ?? t("Préparation")} steps={stepsOf(focused).map(numbered)} active={active} onActive={setActive} />
            ) : (
              <p className="muted">{t("Ouvrez une zone pour voir ses étapes sur sa carte.")}</p>
            )}
          </aside>
        )}
      </div>
    </section>
  );
}
