/** Line icons of the guide actions (16 px, drawn in the text colour). */
const PATHS = {
  // Two arrows going round: compute again.
  rerun: "M13.5 8a5.5 5.5 0 0 1-9.6 3.7M2.5 8a5.5 5.5 0 0 1 9.6-3.7M12.5 1.5v3h-3M3.5 14.5v-3h3",
  // Sliders: the configuration.
  config: "M3 4h10M3 8h10M3 12h10M6 2.5v3M10 6.5v3M5 10.5v3",
  // A pen: the steps by hand.
  steps: "M10.5 2.5l3 3L6 13H3v-3zM9 4l3 3",
  // A box and an arrow out of it: share.
  share: "M8 10V2M5 5l3-3 3 3M3 9v4.5h10V9",
  // Two sheets: clone.
  clone: "M5.5 5.5h8v8h-8zM10.5 5.5v-3h-8v8h3",
  // A flag: report a problem.
  report: "M3.5 14.5v-12M3.5 2.5h8l-2 3 2 3h-8",
  // A bin: delete.
  delete: "M2.5 4h11M6 4V2.5h4V4M4 4l.7 9.5h6.6L12 4M6.8 6.5v5M9.2 6.5v5",
};

export type IconName = keyof typeof PATHS;

export function Icon({ name }: { name: IconName }) {
  return (
    <svg className="icon-svg" viewBox="0 0 16 16" width="16" height="16" aria-hidden fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d={PATHS[name]} />
    </svg>
  );
}
