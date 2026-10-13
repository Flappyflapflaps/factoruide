/** A route the optimization follows: its play time and its score (time plus what makes it
 * tedious: grinding, zone changes, gold spent on flights). The route kept has the best score. */
export interface RaceRoute {
  id: number;
  time: number;
  score: number;
}

/** Live state of the optimization, from the planner's `@progress` messages. */
export interface OptimizeState {
  phase: "prepare" | "construct" | "optimize";
  /** While preparing: what is loading. */
  step?: "world" | "quests";
  /** The race of candidate routes, then the rounds (the better half goes on after each), the
   * last one the final. */
  stage?: "qualify" | "round" | "final";
  /** Candidates of the race (0: no race) and how many are done. */
  candidates?: number;
  qualified?: number;
  /** While constructing: routes built, to build before the race (0: built in the race), and how
   * many of them draw their weights (the others vary the best of those). */
  built?: number;
  to_build?: number;
  draws?: number;
  /** Round under way (from 1) and rounds in all, the final included. */
  round?: number;
  rounds?: number;
  /** Routes of the round, and searches improving each. */
  width?: number;
  searches?: number;
  /** Share of the tries done (0 to 1), and where the race ends; known once a route is built. */
  progress?: number;
  race_end?: number;
  /** Seconds since the race began. */
  elapsed?: number;
  start?: number | null;
  /** Play time of the best route so far (while constructing: of the best route built). */
  best?: number | null;
  /** While racing the best candidates so far, then the routes of the round. */
  routes?: RaceRoute[];
  /** Route kept (best score) among `routes`. */
  selected?: number | null;
}

/** Share of the routes built before the race, or null when they are built in the race. */
export function builtShare(state: OptimizeState | null): number | null {
  if (state?.phase !== "construct" || !state.to_build) return null;
  return Math.min(1, (state.built ?? 0) / state.to_build);
}

/** Times of each followed route, as [share of the tries done, play time] points. */
export type RaceSeries = Record<number, [number, number][]>;

/** Where each round began (share of the tries done) and its routes. */
export interface RaceMark {
  round: number;
  at: number;
  width: number;
}

/** Seconds left, from the pace so far (the tries do not depend on the machine, their pace does). */
export function secondsLeft(state: OptimizeState | null): number | null {
  const done = state?.progress ?? 0;
  if (!state?.elapsed || done < 0.02) return null;
  return Math.max(0, Math.ceil((state.elapsed * (1 - done)) / done));
}

/** Categorical colors (see index.css `--cat-*`); beyond them routes are drawn in gray. */
export const RACE_COLORS = 8;

/** "−45 s", "−3 min", "−1h02". */
export function fmtGain(seconds: number): string {
  if (seconds < 60) return `−${Math.max(1, Math.round(seconds))} s`;
  if (seconds < 3600) return `−${Math.round(seconds / 60)} min`;
  const m = Math.round(seconds / 60);
  return `−${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}`;
}
