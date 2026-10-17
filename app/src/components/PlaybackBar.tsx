import { memo, useCallback, useEffect, useRef, useState } from "react";
import { useT } from "../i18n";
import { fmtClock } from "./guideTimeline";
import { type Clock, SPEEDS, useClock } from "./playback";
import type { Segment } from "./segments";

interface Props {
  clock: Clock;
  segments: Segment[];
  /** The zone under way (index in `segments`). */
  current: number;
  /** Previous and next card. */
  onStep: (delta: number) => void;
}

/** Play, pause, the speed, and the whole guide as a timeline to drag along (its zones on it). */
export function PlaybackBar({ clock, segments, current, onStep }: Props) {
  const t = useT();
  const playing = useClock(clock, (c) => c.playing);
  const speed = useClock(clock, (c) => c.speed);
  return (
    <div className="playback">
      <button className="pb-play" onClick={() => clock.toggle()} aria-label={playing ? t("Pause") : t("Lecture")} title={`${playing ? t("Pause") : t("Lecture")} (${t("Espace")})`}>
        {playing ? "❚❚" : "▶"}
      </button>
      <button onClick={() => onStep(-1)} aria-label={t("Étape précédente")} title={t("Étape précédente")}>
        ⏮
      </button>
      <button onClick={() => onStep(1)} aria-label={t("Étape suivante")} title={t("Étape suivante")}>
        ⏭
      </button>
      <div className="pb-speeds" role="group" aria-label={t("Vitesse")}>
        {SPEEDS.map((s) => (
          <button key={s} className={s === speed ? "active" : undefined} aria-pressed={s === speed} onClick={() => clock.setSpeed(s)} title={s === 1 ? t("Temps réel") : undefined}>
            ×{s}
          </button>
        ))}
      </div>
      <TimeReadout clock={clock} />
      <Scrubber clock={clock} segments={segments} current={current} />
    </div>
  );
}

function TimeReadout({ clock }: { clock: Clock }) {
  const time = useClock(clock, (c) => Math.floor(c.time));
  return (
    <span className="pb-time">
      {fmtClock(time)} <span className="muted">/ {fmtClock(clock.total)}</span>
    </span>
  );
}

/** The guide's time, its zones as blocks; dragging along it moves the time. */
const Scrubber = memo(function Scrubber({ clock, segments, current }: { clock: Clock; segments: Segment[]; current: number }) {
  const t = useT();
  const track = useRef<HTMLDivElement>(null);
  const fill = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<{ x: number; text: string } | null>(null);
  const total = Math.max(1, clock.total);
  // The played part follows the clock without drawing the zones again.
  const draw = useCallback(() => {
    if (fill.current) fill.current.style.width = `${(clock.time / total) * 100}%`;
  }, [clock, total]);
  useEffect(() => {
    draw();
    return clock.subscribe(draw);
  }, [clock, draw]);
  const timeAt = (clientX: number) => {
    const r = track.current!.getBoundingClientRect();
    return (Math.min(r.width, Math.max(0, clientX - r.left)) / r.width) * total;
  };
  const zoneAt = (time: number) => segments.find((s) => s.start <= time && time <= s.end);
  return (
    <div
      className="pb-track"
      ref={track}
      role="slider"
      tabIndex={0}
      aria-label={t("Temps du guide")}
      aria-valuemin={0}
      aria-valuemax={total}
      aria-valuenow={Math.round(clock.time)}
      aria-valuetext={fmtClock(clock.time)}
      onPointerDown={(e) => {
        track.current?.setPointerCapture(e.pointerId);
        clock.seek(timeAt(e.clientX));
      }}
      onPointerMove={(e) => {
        const time = timeAt(e.clientX);
        const r = track.current!.getBoundingClientRect();
        setHover({ x: e.clientX - r.left, text: `${fmtClock(time)} · ${zoneAt(time)?.zone ?? ""}` });
        if (e.buttons & 1) clock.seek(time);
      }}
      onPointerLeave={() => setHover(null)}
      onKeyDown={(e) => {
        if (e.key === "ArrowRight") clock.seek(clock.time + total / 200);
        else if (e.key === "ArrowLeft") clock.seek(clock.time - total / 200);
        else return;
        e.preventDefault();
        e.stopPropagation();
      }}
    >
      {segments.map((s, i) => (
        <i
          key={i}
          className={`pb-zone${i % 2 ? " odd" : ""}${s.dungeon ? " dungeon" : ""}${i === current ? " current" : ""}`}
          style={{ left: `${(s.start / total) * 100}%`, width: `${((s.end - s.start) / total) * 100}%` }}
        />
      ))}
      <div className="pb-fill" ref={fill} />
      {hover && (
        <span className="pb-tip" style={{ left: hover.x }}>
          {hover.text}
        </span>
      )}
    </div>
  );
});
