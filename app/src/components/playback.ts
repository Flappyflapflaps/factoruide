// The game time a guide is watched at, played at a speed: a small store the parts of the viewer
// subscribe to, so that only what changes with the time is drawn again at each frame.
import { useCallback, useSyncExternalStore } from "react";

/** Speeds offered: real time, then faster (game seconds per second). */
export const SPEEDS = [1, 10, 60, 300, 1800];

export class Clock {
  time = 0;
  playing = false;
  speed = 60;
  /** End of the guide (seconds). */
  total: number;
  private listeners = new Set<() => void>();
  private frame = 0;
  private last = 0;

  constructor(total: number) {
    this.total = total;
  }

  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };

  private notify() {
    for (const fn of this.listeners) fn();
  }

  /** Go to a time of the guide (kept within it). */
  seek(time: number) {
    this.time = Math.min(this.total, Math.max(0, time));
    this.notify();
  }

  setSpeed(speed: number) {
    this.speed = speed;
    this.notify();
  }

  play() {
    if (this.playing) return;
    // At the end: from the start again.
    if (this.time >= this.total) this.time = 0;
    this.playing = true;
    this.last = performance.now();
    this.frame = requestAnimationFrame(this.tick);
    this.notify();
  }

  pause() {
    if (!this.playing) return;
    this.playing = false;
    cancelAnimationFrame(this.frame);
    this.notify();
  }

  toggle() {
    if (this.playing) this.pause();
    else this.play();
  }

  private tick = (now: number) => {
    // A frame late after the window was hidden: no jump of minutes.
    const dt = Math.min(0.25, (now - this.last) / 1000);
    this.last = now;
    this.time = Math.min(this.total, this.time + dt * this.speed);
    if (this.time >= this.total) this.playing = false;
    this.notify();
    if (this.playing) this.frame = requestAnimationFrame(this.tick);
  };

  dispose() {
    cancelAnimationFrame(this.frame);
    this.listeners.clear();
  }
}

/** A value read from the clock, the component drawn again only when it changes. */
export function useClock<T>(clock: Clock, select: (c: Clock) => T): T {
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const get = useCallback(() => select(clock), [clock, select]);
  return useSyncExternalStore(clock.subscribe, get);
}
