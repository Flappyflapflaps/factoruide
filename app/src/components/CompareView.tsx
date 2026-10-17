import { Fragment, useEffect, useState, type ReactNode } from "react";
import { api, type Comparison, type DiffBlock, type DiffChange, type DiffRow, type Route, type VersionMeta } from "../api";
import { gameLocale, useDate, useLang, useT } from "../i18n";
import { sections } from "../paramSchema";
import { Delta } from "./ResultView";
import { KIND_LABELS } from "./segments";

interface Props {
  guide: string;
  /** The versions compared: `a` on the left (the older one), `b` on the right. */
  a: number;
  b: number;
  /** Every version of the guide, to compare others. */
  versions: VersionMeta[];
  onChange: (a: number, b: number) => void;
}

/** Identical steps kept around a difference when the others are folded. */
const CONTEXT = 1;

/** What differs in a step (`fg_route::export::diff`, English), French key of the catalog. */
const CHANGE_LABELS: Record<string, string> = {
  text: "texte",
  position: "position",
  goal: "objectif",
  reward: "récompense",
  target: "PNJ",
  "along the way": "en chemin",
};

const STATUS_LABELS: Record<DiffBlock["status"], string> = {
  same: "identique",
  changed: "modifiée",
  added: "ajoutée",
  removed: "supprimée",
  moved: "déplacée",
  moved_away: "déplacée",
};

/** A line of the table: a step on each side (the same one, or a removed one beside an added one). */
type Line = { left: DiffRow | null; right: DiffRow | null } | { run: string; rows: DiffRow[] };

/**
 * Lines of a block: removed and added steps side by side between the steps in both, identical
 * steps far from a difference folded (unless `all`, or their run is in `unfolded`).
 */
function lines(rows: DiffRow[], block: number, all: boolean, unfolded: Set<string>): Line[] {
  const out: Line[] = [];
  let removed: DiffRow[] = [];
  let added: DiffRow[] = [];
  const flush = () => {
    for (let k = 0; k < Math.max(removed.length, added.length); k++) out.push({ left: removed[k] ?? null, right: added[k] ?? null });
    removed = [];
    added = [];
  };
  let k = 0;
  while (k < rows.length) {
    const r = rows[k];
    if (r.op === "removed") removed.push(r);
    else if (r.op === "added") added.push(r);
    else {
      flush();
      if (r.op === "changed") out.push({ left: r, right: r });
      else {
        let end = k;
        while (end < rows.length && rows[end].op === "same") end++;
        const run = rows.slice(k, end);
        const before = k > 0 ? CONTEXT : 0;
        const after = end < rows.length ? CONTEXT : 0;
        const id = `${block}-${k}`;
        if (all || unfolded.has(id) || run.length <= before + after + 1) run.forEach((x) => out.push({ left: x, right: x }));
        else {
          run.slice(0, before).forEach((x) => out.push({ left: x, right: x }));
          out.push({ run: id, rows: run.slice(before, run.length - after) });
          run.slice(run.length - after).forEach((x) => out.push({ left: x, right: x }));
        }
        k = end;
        continue;
      }
    }
    k++;
  }
  flush();
  return out;
}

/** The words of two sentences that differ (what is left between their common start and end). */
function wordDiff(from: string, to: string): [ReactNode, ReactNode] {
  const x = from.split(/(\s+)/);
  const y = to.split(/(\s+)/);
  let start = 0;
  while (start < x.length && start < y.length && x[start] === y[start]) start++;
  let end = 0;
  while (end < x.length - start && end < y.length - start && x[x.length - 1 - end] === y[y.length - 1 - end]) end++;
  const mark = (w: string[]) => (
    <>
      {w.slice(0, start).join("")}
      <mark className="cmp-word">{w.slice(start, w.length - end).join("")}</mark>
      {w.slice(w.length - end).join("")}
    </>
  );
  return [mark(x), mark(y)];
}

/** Two versions of a guide side by side, aligned step by step, their differences marked. */
export function CompareView({ guide, a, b, versions, onChange }: Props) {
  const t = useT();
  const date = useDate();
  const lang = useLang();
  const [cmp, setCmp] = useState<Comparison | null>(null);
  const [error, setError] = useState<string | null>(null);
  // What copying the differences did.
  const [copied, setCopied] = useState<{ ok: boolean; text: string } | null>(null);
  // Only the differences (identical zones and steps folded), or everything.
  const [onlyDiff, setOnlyDiff] = useState(true);
  // Blocks opened or closed by hand (against their default), runs of identical steps or zones unfolded.
  const [toggled, setToggled] = useState<Set<number>>(new Set());
  const [unfolded, setUnfolded] = useState<Set<string>>(new Set());
  // The line jumped to, highlighted a moment.
  const [flash, setFlash] = useState<string | null>(null);

  // Other versions: the parent mounts another view (key).
  useEffect(() => {
    api
      .compareVersions(guide, a, b, gameLocale(lang))
      .then(setCmp)
      .catch((e) => setError(String(e)));
  }, [guide, a, b, lang]);

  useEffect(() => {
    if (!flash) return;
    const el = document.getElementById(flash);
    el?.scrollIntoView({ block: "center" });
    const timer = setTimeout(() => setFlash(null), 1500);
    return () => clearTimeout(timer);
  }, [flash]);

  const blocks = cmp?.diff.blocks ?? [];
  const isOpen = (k: number) => (blocks[k].status !== "same" && blocks[k].status !== "moved_away") !== toggled.has(k);
  const toggle = (k: number) => {
    const next = new Set(toggled);
    if (!next.delete(k)) next.add(k);
    setToggled(next);
  };
  const unfold = (id: string) => setUnfolded(new Set(unfolded).add(id));

  /** Show a step of one side (opening its block) and highlight it. */
  const jump = (side: "a" | "b", index: number) => {
    const k = blocks.findIndex((blk) => blk.rows.some((r) => r[side] === index));
    if (k < 0) return;
    if (!isOpen(k)) toggle(k);
    const group = sameGroups.find((g) => g.includes(k));
    if (group) unfold(`blocks-${group[0]}`);
    setFlash(`cmp-${side}-${index}`);
  };
  /** Show a block (a moved one from where it was, and back). */
  const jumpBlock = (k: number) => {
    const group = sameGroups.find((g) => g.includes(k));
    if (group) unfold(`blocks-${group[0]}`);
    setFlash(`cmp-blk-${k}`);
  };

  // Consecutive identical zones, folded together when only the differences are shown.
  const sameGroups: number[][] = [];
  blocks.forEach((blk, k) => {
    if (blk.status !== "same") return;
    const last = sameGroups[sameGroups.length - 1];
    if (last && last[last.length - 1] === k - 1) last.push(k);
    else sameGroups.push([k]);
  });

  const settingLabel = (field: string) => {
    if (field === "data") return t("Données du jeu");
    const key = field.replace(/^params\./, "");
    const f = sections.flatMap((s) => s.fields).find((x) => x.key === key);
    return f ? t(f.label) : field;
  };
  const versionLabel = (v: VersionMeta) => `v${v.version} · ${date(v.created)} · ${v.total_time_text}`;
  const picker = (value: number, set: (n: number) => void, label: string) => (
    <select aria-label={label} value={value} onChange={(e) => set(Number(e.target.value))}>
      {versions.map((v) => (
        <option key={v.version} value={v.version}>
          {versionLabel(v)}
        </option>
      ))}
    </select>
  );

  const head = (
    <>
      <div className="page-head">
        <h2>{t("Comparer les versions")}</h2>
      </div>
      <div className="actions compare-pickers">
        {picker(a, (n) => onChange(n, b), t("Version de gauche"))}
        <button className="icon" title={t("Inverser")} aria-label={t("Inverser")} onClick={() => onChange(b, a)}>
          ⇄
        </button>
        {picker(b, (n) => onChange(a, n), t("Version de droite"))}
      </div>
    </>
  );
  if (error)
    return (
      <div>
        {head}
        <div className="error">{error}</div>
      </div>
    );
  if (!cmp)
    return (
      <div>
        {head}
        <div className="empty">{t("Chargement…")}</div>
      </div>
    );

  const A = cmp.a.outcome.route;
  const B = cmp.b.outcome.route;
  const s = cmp.diff.summary;
  const same = s.added + s.removed + s.changed + s.moved + s.blocks_moved === 0;
  const kind = (k: string) => t(KIND_LABELS[k] ?? k);
  const zoneOf = (blk: DiffBlock) => {
    const [route, span] = blk.b ? [B, blk.b] : [A, blk.a!];
    const first = route.steps[span.first];
    return (blk.dungeon ? first.target?.name : first.zone) ?? blk.zone ?? t("Préparation");
  };
  const levels = (route: Route, span: { first: number; last: number } | null) => {
    if (!span) return "";
    const [from, to] = [route.steps[span.first].level, route.steps[span.last].level];
    return from === to ? `${from}` : `${from} → ${to}`;
  };
  const spanText = (name: string, span: { first: number; last: number } | null) =>
    span ? `${name} #${span.first + 1}${span.last > span.first ? `–${span.last + 1}` : ""}` : null;
  const [na, nb] = [`v${cmp.a.meta.version}`, `v${cmp.b.meta.version}`];

  /** The three cells of one side of a line. */
  const cells = (side: "a" | "b", row: DiffRow | null) => {
    const index = row ? row[side] : null;
    const split = side === "b" ? " cmp-split" : "";
    if (!row || index == null)
      return (
        <>
          <td className={`cmp-none${split}`} />
          <td className="cmp-none" />
          <td className="cmp-none" />
        </>
      );
    const route = side === "a" ? A : B;
    const step = route.steps[index];
    const moved = row.moved != null;
    const tone =
      row.op === "same" ? "" : row.op === "changed" ? "cmp-changed" : moved ? "cmp-moved" : side === "a" ? "cmp-removed" : "cmp-added";
    // A changed sentence: the words that differ marked on both sides (a moved step: with where it was).
    const changes: DiffChange[] = row.changes ?? [];
    const textChange = changes.find((c) => c.field === "text");
    let text: ReactNode = step.text;
    if (textChange && row.op === "changed") text = wordDiff(A.steps[row.a!].text, B.steps[row.b!].text)[side === "a" ? 0 : 1];
    else if (textChange && moved && side === "b") text = wordDiff(A.steps[row.moved!].text, step.text)[1];
    // A moved step has its differences on its new side only.
    const shownChanges = changes.filter((c) => c.field !== "text" && (row.op === "changed" || side === "b"));
    const id = `cmp-${side}-${index}`;
    const cls = `${tone}${flash === id ? " cmp-flash" : ""}`;
    return (
      <>
        <td className={`num cmp-n ${cls}${split}`} id={id}>
          {index + 1}
        </td>
        <td className={`num ${cls}`}>{step.level}</td>
        <td className={cls}>
          <span className={`kind kind-${step.kind}`}>{kind(step.kind)}</span> {text}
          {step.bg && <span className="along"> {t("en chemin")}</span>}
          {moved && (
            <button className="link cmp-jump" onClick={() => jump(side === "a" ? "b" : "a", row.moved!)}>
              {side === "a" ? t("déplacée → {name} #{n}", { name: nb, n: row.moved! + 1 }) : t("déplacée depuis {name} #{n}", { name: na, n: row.moved! + 1 })}
            </button>
          )}
          {shownChanges.map((c) => (
            <span key={c.field} className="cmp-change" title={`${c.from} → ${c.to}`}>
              {t(CHANGE_LABELS[c.field] ?? c.field)} : {side === "a" ? c.from : c.to}
              {moved && <span className="muted"> ({t("avant : {value}", { value: c.from })})</span>}
            </span>
          ))}
        </td>
      </>
    );
  };

  const blockHeader = (blk: DiffBlock, k: number) => {
    const counts = { added: 0, removed: 0, changed: 0, moved: 0 };
    for (const r of blk.rows) {
      if (r.moved != null) counts.moved += r.op === "added" ? 1 : 0;
      else if (r.op !== "same") counts[r.op] += 1;
    }
    // Where a moved block went or came from.
    const twin =
      blk.status === "moved" || blk.status === "moved_away"
        ? blocks.findIndex((x, i) => i !== k && (x.status === "moved" || x.status === "moved_away") && x.a?.first === blk.a?.first)
        : -1;
    const open = blk.status !== "moved_away" && isOpen(k);
    return (
      <tr
        id={`cmp-blk-${k}`}
        className={`clickable zone-line cmp-block cmp-st-${blk.status}${flash === `cmp-blk-${k}` ? " cmp-flash" : ""}`}
        onClick={() => (blk.status === "moved_away" ? twin >= 0 && jumpBlock(twin) : toggle(k))}
      >
        <td colSpan={6}>
          {blk.status !== "moved_away" && (
            <button className="disclosure" aria-expanded={open} aria-label={open ? t("Replier") : t("Déplier")}>
              {open ? "▾" : "▸"}
            </button>
          )}{" "}
          <span className={`tag cmp-tag cmp-st-${blk.status}`}>{t(STATUS_LABELS[blk.status])}</span>{" "}
          {blk.dungeon && <span className="kind kind-dungeon">{t("Donjon")}</span>} <strong>{zoneOf(blk)}</strong>{" "}
          <span className="muted">
            {blk.status === "moved_away"
              ? twin >= 0 && (twin > k ? t("plus loin dans {name} ↓", { name: nb }) : t("plus tôt dans {name} ↑", { name: nb }))
              : [spanText(na, blk.a), spanText(nb, blk.b)].filter(Boolean).join(" | ")}
            {blk.status === "moved" && twin >= 0 && (
              <button
                className="link cmp-jump"
                onClick={(e) => {
                  e.stopPropagation();
                  jumpBlock(twin);
                }}
              >
                {twin > k ? t("était plus loin dans {name} ↓", { name: na }) : t("était plus tôt dans {name} ↑", { name: na })}
              </button>
            )}
            {" · "}
            {t("niveaux {levels}", { levels: levels(blk.b ? B : A, blk.b ?? blk.a) })}
          </span>
          {blk.status !== "moved_away" && blk.status !== "same" && (
            <span className="cmp-counts">
              {counts.added > 0 && <span className="cmp-count-added">+{counts.added}</span>}
              {counts.removed > 0 && <span className="cmp-count-removed">−{counts.removed}</span>}
              {counts.changed > 0 && <span className="cmp-count-changed">~{counts.changed}</span>}
              {counts.moved > 0 && <span className="cmp-count-moved">↔{counts.moved}</span>}
            </span>
          )}
        </td>
      </tr>
    );
  };

  const blockRows = (blk: DiffBlock, k: number) =>
    lines(blk.rows, k, !onlyDiff, unfolded).map((line, n) =>
      "run" in line ? (
        <tr key={n} className="sub-row clickable cmp-run" onClick={() => unfold(line.run)}>
          <td colSpan={6}>{t("… {n} étapes identiques (afficher)", { n: line.rows.length })}</td>
        </tr>
      ) : (
        <tr key={n} className="sub-row">
          {cells("a", line.left)}
          {cells("b", line.right)}
        </tr>
      ),
    );

  const body: ReactNode[] = [];
  for (let k = 0; k < blocks.length; k++) {
    const group = onlyDiff ? sameGroups.find((g) => g[0] === k) : undefined;
    if (group && group.length > 1 && !unfolded.has(`blocks-${k}`)) {
      const names = [...new Set(group.map((i) => zoneOf(blocks[i])))];
      body.push(
        <tr key={`g${k}`} className="clickable cmp-run cmp-same-zones" onClick={() => unfold(`blocks-${k}`)}>
          <td colSpan={6}>
            {t("{n} zones identiques", { n: group.length })} <span className="muted">· {names.slice(0, 6).join(", ")}{names.length > 6 ? "…" : ""}</span>
          </td>
        </tr>,
      );
      k = group[group.length - 1];
      continue;
    }
    const blk = blocks[k];
    body.push(
      <Fragment key={k}>
        {blockHeader(blk, k)}
        {blk.status !== "moved_away" && isOpen(k) && blockRows(blk, k)}
      </Fragment>,
    );
  }

  // A selected text copied at once (the clipboard API waits for the window's focus in WebView2).
  const copy = () => {
    const done = () => setCopied({ ok: true, text: t("Différences copiées en texte : collez-les à un agent ou dans un signalement.") });
    const area = document.createElement("textarea");
    area.value = cmp.text;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand("copy");
    area.remove();
    if (ok) done();
    else
      navigator.clipboard
        .writeText(cmp.text)
        .then(done)
        .catch((e) => setCopied({ ok: false, text: String(e) }));
  };

  return (
    <div className="compare-view">
      {head}
      <div className="tiles">
        <div className="tile tile-hero">
          <span>{t("Temps de jeu estimé")}</span>
          <strong>
            {A.total_time_text} → {B.total_time_text}
          </strong>
          <small>
            <Delta seconds={B.total_time - A.total_time} />
          </small>
        </div>
        <div className="tile">
          <span>{t("Étapes")}</span>
          <strong>
            {A.steps.length} → {B.steps.length}
          </strong>
          <small>{t("{a} → {b} quêtes", { a: A.quests, b: B.quests })}</small>
        </div>
        <div className="tile">
          <span>{t("Étapes différentes")}</span>
          <strong className="cmp-counts">
            <span className="cmp-count-added">+{s.added}</span> <span className="cmp-count-removed">−{s.removed}</span>{" "}
            <span className="cmp-count-changed">~{s.changed}</span>
          </strong>
          <small>{t("{n} déplacées", { n: s.moved })}</small>
        </div>
        <div className="tile">
          <span>{t("Zones déplacées")}</span>
          <strong>{s.blocks_moved}</strong>
          <small>{t("{added} ajoutées · {removed} supprimées", { added: s.blocks_added, removed: s.blocks_removed })}</small>
        </div>
      </div>

      {cmp.settings.length > 0 && (
        <details className="notes">
          <summary>{t("Réglages différents ({n})", { n: cmp.settings.length })}</summary>
          <table className="table compact">
            <tbody>
              {cmp.settings.map((c) => (
                <tr key={c.field}>
                  <td title={c.field}>{settingLabel(c.field)}</td>
                  <td>{c.from}</td>
                  <td>→ {c.to}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </details>
      )}

      {copied && <div className={copied.ok ? "success" : "error"}>{copied.text}</div>}
      <div className="route-toolbar">
        <label className="cmp-only">
          <input type="checkbox" checked={onlyDiff} onChange={(e) => setOnlyDiff(e.target.checked)} /> {t("Seulement les différences")}
        </label>
        <span className="cmp-spacer" />
        <button onClick={copy} title={t("Le texte des différences, pour chercher une erreur avec un agent ou la signaler")}>
          {t("Copier les différences (texte)")}
        </button>
      </div>

      {same ? (
        <div className="notice">{t("Les deux versions ont les mêmes étapes.")}</div>
      ) : (
        <table className="table route-table compare-table">
          <colgroup>
            <col className="c-n" />
            <col className="c-lv" />
            <col />
            <col className="c-n" />
            <col className="c-lv" />
            <col />
          </colgroup>
          <thead>
            <tr>
              <th colSpan={3}>
                {na} <span className="muted">· {date(cmp.a.meta.created)}</span>
              </th>
              <th colSpan={3} className="cmp-split">
                {nb} <span className="muted">· {date(cmp.b.meta.created)}</span>
              </th>
            </tr>
          </thead>
          <tbody>{body}</tbody>
        </table>
      )}
    </div>
  );
}
