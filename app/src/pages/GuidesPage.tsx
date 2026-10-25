import { useCallback, useEffect, useRef, useState } from "react";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { api, type Edition, type GuideSummary, type GuideVersion, type Options, type VersionMeta } from "../api";
import { ExportMenu, GlobalExport, type ExportResult } from "../components/ExportMenu";
import "./guides.css";
import { Character } from "../components/Character";
import { CompareView } from "../components/CompareView";
import { ResultView } from "../components/ResultView";
import { gameLocale, useDate, useLang, useT } from "../i18n";
import { Icon } from "../components/Icons";
import { RxpImport } from "../components/RxpImport";
import { IS_WEB } from "../web/mode";

export interface OpenGuide {
  guide: string;
  version: number;
  notice: string | null;
  /** Open with its steps edited (a guide created by hand). */
  edit?: boolean;
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
  /** The game version the app works on. */
  edition: Edition | null;
  /** Open the example guide (added first); only on the game versions that have one. */
  onExample?: () => Promise<void>;
}

/** The version a box shows and the global export sends: the one chosen (the latest unless one is chosen). */
const shownVersion = (g: GuideSummary): VersionMeta => g.versions.find((v) => v.version === g.installed) ?? g.versions[0];

/** Every guide with its versions, who follows it, and the actions of its page on each row. */
export function GuidesPage({ options, start, onRerun, onEdit, onClone, onUnsaved, edition, onExample }: Props) {
  const t = useT();
  const date = useDate();
  const [guides, setGuides] = useState<GuideSummary[] | null>(null);
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
  // The guide opens with its steps edited (from the list's button, or created by hand).
  const [editOnOpen, setEditOnOpen] = useState(!!start?.edit);
  const leave = () => !unsaved.current || confirm(t("Quitter l'édition sans enregistrer ? Les modifications seront perdues."));
  // Two versions of the open guide compared (from its page, back to it).
  const [comparing, setComparing] = useState<{ a: number; b: number } | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(start?.notice ?? null);
  // Opening `start`: no list in the meantime.
  const [pending, setPending] = useState(!!start);
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  // The race and class asked of a guide made from RestedXP's (desktop app: it reads the game's addons).
  const [rxp, setRxp] = useState(false);
  // The same, from Polestar's leveling guides (WoW Forever, desktop app: it reads a folder).
  const [polestar, setPolestar] = useState(false);

  const load = useCallback(() => {
    api.listGuides().then(setGuides).catch((e) => setError(String(e)));
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

  /** What an export said (shown over the page). */
  const exported = (r: ExportResult) => {
    setError(r.error ?? null);
    setMessage(r.message || null);
  };

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
          edition={edition}
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
      <div className="guides-head-actions">
        <button onClick={() => fileRef.current?.click()} title={t("Ajoute un guide reçu d'un autre joueur (fichier .fgguide)")}>
          {t("Importer un guide…")}
        </button>
        {!IS_WEB && options && (
          <button
            onClick={() => {
              setRxp(!rxp);
              setPolestar(false);
            }}
            title={t("Fait un guide des guides RestedXP de l'addon RestedXP Guides installé dans le jeu")}
          >
            {t("Importer de RestedXP…")}
          </button>
        )}
        {!IS_WEB && options && edition === "forever" && (
          <button
            onClick={() => {
              setPolestar(!polestar);
              setRxp(false);
            }}
            title={t("Fait un guide des guides de montée de niveau de Polestar (le dossier guides d'un téléchargement de Polestar)")}
          >
            {t("Importer de Polestar…")}
          </button>
        )}
        {!!guides?.length && <GlobalExport edition={edition} count={guides.length} onDone={exported} />}
      </div>
      {rxp && options && (
        <RxpImport
          options={options}
          onClose={() => setRxp(false)}
          onImport={(race, klass) => {
            setRxp(false);
            act(async () => t("Guide « {name} » importé", { name: (await api.importRxp(race, klass)).name }));
          }}
        />
      )}
      {polestar && options && (
        <RxpImport
          options={options}
          intro={t(
            "Fait un guide des guides de montée de niveau de Polestar (GPL-3.0), rejoués avec les données de Factoruide : choisissez ensuite le dossier guides d'un téléchargement de Polestar (github.com/fitzforge/polestar). Les étapes impossibles sont retirées et le guide crédite Polestar.",
          )}
          onClose={() => setPolestar(false)}
          onImport={async (race, klass) => {
            const dir = await pickFolder({ directory: true, title: t("Dossier guides de Polestar") });
            if (typeof dir !== "string") return;
            setPolestar(false);
            act(async () => t("Guide « {name} » importé", { name: (await api.importPolestar(race, klass, dir)).name }));
          }}
        />
      )}
    </div>
  );
  const notices = (
    <>
      {message && <div className="success">{message}</div>}
      {error && <div className="error">{error}</div>}
    </>
  );
  if (pending) return <div className="empty">{t("Chargement…")}</div>;
  if (!guides) return error ? notices : <div className="empty">{t("Chargement…")}</div>;
  if (guides.length === 0)
    return (
      <div className="results">
        {bar}
        {notices}
        <div className="empty">
          {t("Aucun résultat pour l'instant : lancez une génération depuis « Nouveau guide ».")}
          {onExample && (
            <p>
              <button
                onClick={() => {
                  setPending(true);
                  setError(null);
                  onExample().catch((e) => {
                    setPending(false);
                    setError(String(e));
                  });
                }}
                title={t("Un guide déjà calculé (humain paladin, niveaux 1 à 30) pour découvrir la carte, les étapes et l'éditeur.")}
              >
                {t("Voir un guide d'exemple")}
              </button>
            </p>
          )}
        </div>
      </div>
    );

  return (
    <div className="results">
      {bar}
      {notices}
      <div className="guide-cards">
        {guides.map((g) => {
          const latest = g.versions[0];
          const shown = shownVersion(g);
          return (
            <article key={g.id} className="panel guide-card">
              <header>
                <button className="link guide-card-name" onClick={() => openVersion(g.id, shown.version)}>
                  {g.name}
                </button>
                <div className="guide-card-tags">
                  <span className="tag" title={t("Version envoyée au jeu par l'export global")}>
                    v{shown.version}
                    {g.versions.length > 1 ? ` / ${latest.version}` : ""}
                  </span>
                  {latest.imported && <span className="tag">{t("importé")}</span>}
                  {latest.edited && <span className="tag">{t("modifié à la main")}</span>}
                </div>
              </header>
              <div className="guide-card-who">
                <Character race={latest.race} klass={latest.class} group={latest.group} options={options} />
                {latest.character && (
                  <span className="muted" title={t("Part du personnage {name} tel qu'enregistré en jeu", { name: latest.character })}>
                    {t("depuis {name}", { name: latest.character.split("-")[0] })}
                  </span>
                )}
              </div>
              <dl className="guide-card-facts">
                <div>
                  <dt>{t("Niveaux")}</dt>
                  <dd>
                    {shown.from_level} → {shown.to_level}
                  </dd>
                </div>
                <div>
                  <dt>{t("Temps estimé")}</dt>
                  <dd>{shown.total_time_text}</dd>
                </div>
                <div>
                  <dt>{t("Quêtes")}</dt>
                  <dd>{shown.quests}</dd>
                </div>
              </dl>
              <p className="muted guide-card-date">{t("Calculé le {date}", { date: date(latest.created) })}</p>
              <footer>
                <button className="primary" onClick={() => openVersion(g.id, shown.version)}>
                  {t("Ouvrir")}
                </button>
                <ExportMenu guide={g.id} version={shown.version} edition={edition} onDone={exported} />
                <span className="guide-card-icons">
                  {/* The actions of the guide's page, on the version the box shows. */}
                  <button className="icon" title={t("Relancer le calcul")} aria-label={t("Relancer le calcul")} onClick={() => withVersion(g, onRerun)}>
                    <Icon name="rerun" />
                  </button>
                  <button className="icon" title={t("Modifier la configuration")} aria-label={t("Modifier la configuration")} onClick={() => withVersion(g, onEdit)}>
                    <Icon name="config" />
                  </button>
                  <button
                    className="icon"
                    title={t("Modifier les étapes")}
                    aria-label={t("Modifier les étapes")}
                    onClick={() =>
                      withVersion(g, (v) => {
                        setEditOnOpen(true);
                        setOpen(v);
                      })
                    }
                  >
                    <Icon name="steps" />
                  </button>
                  <button className="icon" title={t("Nouveau guide à partir de la même configuration")} aria-label={t("Cloner")} onClick={() => withVersion(g, onClone)}>
                    <Icon name="clone" />
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
                </span>
              </footer>
            </article>
          );
        })}
      </div>
      <p className="muted list-hint">{t("Créez un guide dans « Nouveau guide », ou importez celui d'un autre joueur (fichier .fgguide).")}</p>
    </div>
  );
}
