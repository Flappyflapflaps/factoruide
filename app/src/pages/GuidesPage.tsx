import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, type GuideSummary, type GuideVersion, type Options, type VersionMeta } from "../api";
import { Character } from "../components/Character";
import { CompareView } from "../components/CompareView";
import { ResultView } from "../components/ResultView";
import { gameLocale, useDate, useGameName, useLang, useT } from "../i18n";
import { Icon } from "../components/Icons";

export interface OpenGuide {
  guide: string;
  version: number;
  notice: string | null;
}

interface Props {
  /** Options of the game version (race names and factions), when loaded. */
  options: Options | null;
  /** A version to open at once instead of the list. */
  start?: OpenGuide | null;
  onRerun: (version: GuideVersion) => void;
  onEdit: (version: GuideVersion) => void;
  /** A new guide from the same configuration (under another name). */
  onClone: (version: GuideVersion) => void;
  /** The steps of the guide open are edited and not saved. */
  onUnsaved?: (unsaved: boolean) => void;
}

type SortKey = "addon" | "name" | "character" | "levels" | "time" | "versions" | "created";

/** The version a row shows: the one its characters follow (the latest unless one is chosen). */
const shownVersion = (g: GuideSummary): VersionMeta => g.versions.find((v) => v.version === g.installed) ?? g.versions[0];

/** Every guide with its versions, who follows it, and the actions of its page on each row. */
export function GuidesPage({ options, start, onRerun, onEdit, onClone, onUnsaved }: Props) {
  const t = useT();
  const date = useDate();
  const gameName = useGameName();
  const [guides, setGuides] = useState<GuideSummary[] | null>(null);
  // Which guide each character follows ("Name-Realm" -> guide).
  const [assignments, setAssignments] = useState<Record<string, string>>({});
  const followers = (id: string) => Object.keys(assignments).filter((c) => assignments[c] === id);
  // Column sorted by (none: the saved order), ascending or not.
  const [sort, setSort] = useState<{ key: SortKey; asc: boolean } | null>(null);
  const [open, setOpen] = useState<GuideVersion | null>(null);
  // The guide open has its steps edited and not saved: leaving it asks first.
  const unsaved = useRef(false);
  const onEdited = useCallback(
    (u: boolean) => {
      unsaved.current = u;
      onUnsaved?.(u);
    },
    [onUnsaved],
  );
  // The guide opens with its steps edited (from the list's button).
  const [editOnOpen, setEditOnOpen] = useState(false);
  const leave = () => !unsaved.current || confirm(t("Quitter l'édition sans enregistrer ? Les modifications seront perdues."));
  // Two versions of the open guide compared (from its page, back to it).
  const [comparing, setComparing] = useState<{ a: number; b: number } | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(start?.notice ?? null);
  // Opening `start`: no list in the meantime.
  const [pending, setPending] = useState(!!start);
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const load = useCallback(() => {
    api.listGuides().then(setGuides).catch((e) => setError(String(e)));
    api.listAssignments().then(setAssignments).catch(() => {});
  }, []);
  useEffect(load, [load]);

  const act = async (action: () => Promise<string | void>) => {
    setError(null);
    setMessage(null);
    try {
      const m = await action();
      if (m) setMessage(m);
      load();
    } catch (e) {
      setError(String(e));
    }
  };
  const lang = useLang();
  /** Run an action on the version a row shows (loaded first). */
  const withVersion = (g: GuideSummary, action: (v: GuideVersion) => void) =>
    api
      .getVersion(g.id, shownVersion(g).version, gameLocale(lang))
      .then(action)
      .catch((e) => setError(String(e)));
  const openVersion = (guide: string, version: number) =>
    api
      .getVersion(guide, version, gameLocale(lang))
      .then(setOpen)
      .catch((e) => setError(String(e)));
  useEffect(() => {
    if (start) openVersion(start.guide, start.version).finally(() => setPending(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const sorted = useMemo(() => {
    if (!guides || !sort) return guides;
    const character = (v: VersionMeta) => `${gameName(options?.races.find((r) => r.key === v.race)?.name ?? v.race)} ${gameName(v.class)}`;
    const compare: Record<SortKey, (a: GuideSummary, b: GuideSummary) => number> = {
      addon: (a, b) => followers(a.id).length - followers(b.id).length,
      name: (a, b) => a.name.localeCompare(b.name),
      character: (a, b) => character(a.versions[0]).localeCompare(character(b.versions[0])),
      levels: (a, b) => shownVersion(a).from_level - shownVersion(b).from_level || shownVersion(a).to_level - shownVersion(b).to_level,
      time: (a, b) => shownVersion(a).total_time - shownVersion(b).total_time,
      versions: (a, b) => a.versions[0].version - b.versions[0].version,
      created: (a, b) => a.versions[0].created - b.versions[0].created,
    };
    return [...guides].sort((a, b) => (sort.asc ? 1 : -1) * compare[sort.key](a, b) || a.name.localeCompare(b.name));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [guides, sort, gameName, options, assignments]);

  /** A column header sorting the table: ascending first, then descending. */
  const header = (key: SortKey, label: string, title?: string) => (
    <th aria-sort={sort?.key === key ? (sort.asc ? "ascending" : "descending") : "none"} title={title}>
      <button className="sort" onClick={() => setSort(sort?.key === key ? { key, asc: !sort.asc } : { key, asc: true })}>
        {label}
        <span className="sort-arrow" aria-hidden="true">
          {sort?.key === key ? (sort.asc ? "▲" : "▼") : "↕"}
        </span>
      </button>
    </th>
  );

  if (open && comparing) {
    return (
      <div>
        <button className="link" onClick={() => setComparing(null)}>
          {t("← Version {n}", { n: open.meta.version })}
        </button>
        <CompareView
          key={`${comparing.a}-${comparing.b}`}
          guide={open.meta.guide}
          a={comparing.a}
          b={comparing.b}
          versions={guides?.find((g) => g.id === open.meta.guide)?.versions ?? []}
          onChange={(a, b) => setComparing({ a, b })}
        />
      </div>
    );
  }

  if (open) {
    return (
      <div>
        <button
          className="link"
          onClick={() => {
            if (!leave()) return;
            setEditOnOpen(false);
            setOpen(null);
            setMessage(null);
            setNotice(null);
            load();
          }}
        >
          {t("← Tous les guides")}
        </button>
        {message && <div className="success">{message}</div>}
        {notice && <div className="notice">{notice}</div>}
        <ResultView
          key={`${open.meta.guide}-${open.meta.version}`}
          version={open}
          options={options}
          onRerun={onRerun}
          onEdit={onEdit}
          onClone={onClone}
          onUnsaved={onEdited}
          editing={editOnOpen}
          onSaved={(meta) => {
            setEditOnOpen(false);
            setMessage(t("Guide modifié enregistré : version {n}", { n: meta.version }));
            openVersion(meta.guide, meta.version);
          }}
          onOpenVersion={(n) => {
            if (!leave()) return;
            setEditOnOpen(false);
            openVersion(open.meta.guide, n);
          }}
          onCompare={(a, b) => {
            if (!leave()) return;
            // The versions made since the list was loaded.
            load();
            setComparing({ a, b });
          }}
        />
      </div>
    );
  }

  const bar = (
    <div className="page-head">
      <h2>{t("Guides")}</h2>
      <input
        ref={fileRef}
        type="file"
        accept=".fgguide,.bqguide,.bqroute,application/json"
        hidden
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) act(async () => t("Guide « {name} » importé", { name: (await api.importGuide(await file.text())).name }));
          e.target.value = "";
        }}
      />
      <button onClick={() => fileRef.current?.click()} title={t("Ajoute un guide reçu d'un autre joueur (fichier .fgguide)")}>
        {t("Importer un guide…")}
      </button>
    </div>
  );
  const notices = (
    <>
      {message && <div className="success">{message}</div>}
      {error && <div className="error">{error}</div>}
    </>
  );
  if (pending) return <div className="empty">{t("Chargement…")}</div>;
  if (!guides || !sorted) return error ? notices : <div className="empty">{t("Chargement…")}</div>;
  if (guides.length === 0)
    return (
      <div className="results">
        {bar}
        {notices}
        <div className="empty">{t("Aucun résultat pour l'instant : lancez une génération depuis « Nouveau guide ».")}</div>
      </div>
    );

  return (
    <div className="results">
      {bar}
      {notices}
      <table className="table">
        <thead>
          <tr>
            {header("addon", t("Suivi par"), t("Personnages qui suivent ce guide : à choisir dans la page Personnages."))}
            {header("name", t("Guide"))}
            {header("character", t("Personnage"))}
            {header("levels", t("Niveaux"))}
            {header("time", t("Temps estimé"))}
            {header("versions", t("Versions"))}
            {header("created", t("Calculé le"))}
            <th>
              <span className="sr-only">{t("Actions")}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((g) => {
            const latest = g.versions[0];
            const shown = shownVersion(g);
            return (
              <tr key={g.id}>
                <td className="followers">
                  {followers(g.id).map((c) => (
                    <span key={c} className="tag" title={c}>
                      {c.split("-")[0]}
                    </span>
                  ))}
                </td>
                <td>
                  <button className="link" onClick={() => openVersion(g.id, shown.version)}>
                    {g.name}
                  </button>
                  {g.installed != null && g.installed !== latest.version && (
                    <span className="tag" title={t("Une version plus récente existe")}>
                      {t("v{n} suivie", { n: g.installed })}
                    </span>
                  )}
                  {latest.imported && <span className="tag">{t("importé")}</span>}
                  {latest.character && (
                    <span className="tag" title={t("Part du personnage {name} tel qu'enregistré en jeu", { name: latest.character })}>
                      {latest.character.split("-")[0]}
                    </span>
                  )}
                </td>
                <td>
                  <Character race={latest.race} klass={latest.class} group={latest.group} options={options} />
                </td>
                <td>
                  {shown.from_level} → {shown.to_level}
                </td>
                <td>{shown.total_time_text}</td>
                <td>v{latest.version}</td>
                <td>{date(latest.created)}</td>
                <td className="row-icons">
                  {/* The actions of the guide's page, on the version the row shows. */}
                  <button className="icon" title={t("Relancer le calcul")} aria-label={t("Relancer le calcul")} onClick={() => withVersion(g, onRerun)}>
                    <Icon name="rerun" />
                  </button>
                  <button className="icon" title={t("Modifier la configuration")} aria-label={t("Modifier la configuration")} onClick={() => withVersion(g, onEdit)}>
                    <Icon name="config" />
                  </button>
                  <button className="icon" title={t("Modifier les étapes")} aria-label={t("Modifier les étapes")} onClick={() => withVersion(g, (v) => {
                        setEditOnOpen(true);
                        setOpen(v);
                      })}>
                    <Icon name="steps" />
                  </button>
                  <button className="icon" title={t("Nouveau guide à partir de la même configuration")} aria-label={t("Cloner")} onClick={() => withVersion(g, onClone)}>
                    <Icon name="clone" />
                  </button>
                  <button
                    className="icon"
                    title={t("Enregistre un fichier .fgguide (configuration et guide) à envoyer aux autres joueurs")}
                    aria-label={t("Partager")}
                    onClick={() => act(async () => t("Fichier à partager enregistré : {path}", { path: await api.exportVersion(g.id, shown.version) }))}
                  >
                    <Icon name="share" />
                  </button>
                  <button
                    className="icon danger"
                    title={t("Supprimer")}
                    aria-label={t("Supprimer")}
                    onClick={() => {
                      if (confirm(t("Supprimer le guide « {name} » et ses {n} versions ?", { name: g.name, n: g.versions.length }))) act(() => api.deleteGuide(g.id));
                    }}
                  >
                    <Icon name="delete" />
                  </button>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <p className="muted list-hint">{t("Créez un guide dans « Nouveau guide », ou importez celui d'un autre joueur (fichier .fgguide).")}</p>
    </div>
  );
}
