/** An amount of copper as the game shows money: gold, silver and copper coins (the zero ones left out). */
export function Money({ copper }: { copper: number }) {
  const gold = Math.floor(copper / 10000);
  const silver = Math.floor((copper % 10000) / 100);
  const rest = copper % 100;
  const parts: [number, string][] = [
    [gold, "gold"],
    [silver, "silver"],
    [rest, "copper"],
  ];
  // Copper only when it is all there is; silver only below 100 gold.
  const shown = parts.filter(([n, kind], i) => n > 0 && (kind !== "copper" || (gold === 0 && silver === 0)) && !(i === 1 && gold >= 100));
  return (
    <span className="money">
      {(shown.length ? shown : [[0, "copper"] as [number, string]]).map(([n, kind]) => (
        <span key={kind}>
          {n}
          <i className={`coin ${kind}`} aria-hidden />
          <span className="sr-only">{kind === "gold" ? "g" : kind === "silver" ? "s" : "c"}</span>
        </span>
      ))}
    </span>
  );
}
