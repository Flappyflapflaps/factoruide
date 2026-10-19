import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type Edition } from "../api";
import { useT } from "../i18n";
import { IS_WEB } from "../web/mode";
import { Icon } from "./Icons";
import "./export.css";

/** A button opening a small menu of choices (closed by a click elsewhere or Escape). */
function Menu({ label, title, className, items }: { label: string; title?: string; className?: string; items: { label: string; hint: string; run: () => void }[] }) {
  const [shown, setShown] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!shown) return;
    const away = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setShown(false);
    };
    const escape = (e: KeyboardEvent) => e.key === "Escape" && setShown(false);
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", escape);
    };
  }, [shown]);
  return (
    <div className="export-menu" ref={root}>
      <button className={className} title={title} aria-haspopup="menu" aria-expanded={shown} onClick={() => setShown(!shown)}>
        <Icon name="share" /> {label} ▾
      </button>
      {shown && (
        <div className="export-menu-list" role="menu">
          {items.map((item) => (
            <button
              key={item.label}
              role="menuitem"
              onClick={() => {
                setShown(false);
                item.run();
              }}
            >
              <strong>{item.label}</strong>
              <small>{item.hint}</small>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** What an export did, or why it failed. */
export type ExportResult = { message?: string; error?: string };

interface GuideProps {
  guide: string;
  version: number;
  edition: Edition | null;
  /** A bigger button (the guide's page). */
  prominent?: boolean;
  onDone: (result: ExportResult) => void;
}

/** One guide version: for the app (.fgguide), for the addon (the text of the guide import), for Polestar (WoW Forever). */
export function ExportMenu({ guide, version, edition, prominent, onDone }: GuideProps) {
  const t = useT();
  const [text, setText] = useState<string | null>(null);
  const run = (action: () => Promise<string>) =>
    action()
      .then((message) => onDone({ message }))
      .catch((e) => onDone({ error: String(e) }));
  const items = [
    {
      label: t("Addon"),
      hint: t("Le texte à coller en jeu (/fg puis « Importer un guide… ») : le guide rejoint ceux de l'addon."),
      run: () => {
        setText("");
        api
          .guideText(guide, version)
          .then(setText)
          .catch((e) => {
            setText(null);
            onDone({ error: String(e) });
          });
      },
    },
    {
      label: t("Application"),
      hint: t("Un fichier .fgguide (configuration et guide) à ouvrir dans Factoruide, à envoyer aux autres joueurs."),
      run: () => run(async () => t("Fichier à partager enregistré : {path}", { path: await api.exportVersion(guide, version) })),
    },
    ...(edition === "forever"
      ? [
          {
            label: "Polestar",
            hint: t("Le guide au format .guide de Polestar (en anglais), pour ses outils Planner et Play."),
            run: () =>
              run(async () => {
                const dir = await open({ directory: true, title: t("Dossier où exporter le pack Polestar") });
                if (typeof dir !== "string") return "";
                const path = await api.exportPolestar(guide, version, dir);
                return IS_WEB
                  ? t("Pack Polestar téléchargé ({path}) : décompressez-le et ouvrez son dossier dans Planner ou Play de Polestar.", { path })
                  : t("Guide exporté pour Polestar dans {path} : ouvrez ce dossier dans Planner ou Play de Polestar.", { path });
              }),
          },
        ]
      : []),
  ];
  return (
    <>
      <Menu label={t("Exporter")} title={t("Exporter ce guide : pour l'addon, pour l'application, pour Polestar")} className={prominent ? "primary export-prominent" : undefined} items={items} />
      {text !== null && <GuideTextDialog text={text} onClose={() => setText(null)} />}
    </>
  );
}

/** The text of the guide import of a guide, to copy. */
function GuideTextDialog({ text, onClose }: { text: string; onClose: () => void }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setError(t("Le navigateur refuse la copie : sélectionnez le texte et copiez-le (Ctrl+C)."));
    }
  };
  return (
    <div className="welcome-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="panel export-dialog" role="dialog" aria-modal="true" aria-labelledby="export-dialog-title">
        <h2 id="export-dialog-title">{t("Exporter pour l'addon")}</h2>
        {!text ? (
          <p className="muted">{t("Préparation…")}</p>
        ) : (
          <>
            <ol className="export-steps">
              <li>{t("Copiez le texte ci-dessous.")}</li>
              <li>{t("En jeu, tapez /fg, cliquez sur « Importer un guide… », collez-le (Ctrl+V), puis Importer.")}</li>
              <li>{t("Choisissez le guide de votre personnage dans la fenêtre /fg, page Guides (d'office s'il est le seul de sa race et de sa classe).")}</li>
            </ol>
            <div className="row">
              <button className="primary" onClick={copy}>
                {copied ? t("Copié ✓") : t("Copier le texte")}
              </button>
              <span className="muted">{t("{size} Ko", { size: Math.ceil(text.length / 1024) })}</span>
            </div>
            <textarea className="export-text" readOnly value={text} onFocus={(e) => e.currentTarget.select()} spellCheck={false} />
          </>
        )}
        {error && <div className="error">{error}</div>}
        <div className="setup-actions">
          <span />
          <button onClick={onClose}>{t("Fermer")}</button>
        </div>
      </div>
    </div>
  );
}

/** Every guide at once: for the addon (the data addon), for Polestar (WoW Forever). */
export function GlobalExport({ edition, count, onDone }: { edition: Edition | null; count: number; onDone: (result: ExportResult) => void }) {
  const t = useT();
  const run = (action: () => Promise<string>) =>
    action()
      .then((message) => onDone({ message }))
      .catch((e) => onDone({ error: String(e) }));
  const items = [
    IS_WEB
      ? {
          label: t("Addon : tous les guides"),
          hint: t("Le dossier Factoruide_Data (.zip) à décompresser dans le dossier Interface\\AddOns du jeu, puis redémarrer le jeu."),
          run: () => run(async () => t("Téléchargé : {file}. Décompressez-le dans Interface\\AddOns, puis redémarrez le jeu.", { file: await api.dataZip() })),
        }
      : {
          label: t("Addon : tous les guides dans le jeu"),
          hint: t("Écrit vos guides dans le dossier du jeu (Factoruide_Data), puis /reload en jeu (redémarrer le jeu la première fois)."),
          run: () => run(async () => t("Guides écrits dans le jeu : {n}. Faites /reload en jeu (redémarrez le jeu la première fois).", { n: await api.installAddon() })),
        },
    ...(edition === "forever"
      ? [
          IS_WEB
            ? {
                label: t("Polestar : tous les guides"),
                hint: t("Le pack de guides Polestar (.zip) à décompresser dans le dossier Interface\\AddOns de WoW Forever."),
                run: () => run(async () => t("Téléchargé : {file}. Décompressez-le dans Interface\\AddOns, puis redémarrez le jeu.", { file: await api.polestarPack() })),
              }
            : {
                label: t("Polestar : tous les guides dans le jeu"),
                hint: t("Écrit vos guides comme pack de guides Polestar (Factoruide_Polestar) dans le dossier du jeu."),
                run: () => run(async () => t("Guides écrits pour Polestar : {n}. Redémarrez le jeu.", { n: await api.installPolestar() })),
              },
        ]
      : []),
  ];
  return <Menu label={t("Export global")} title={t("Tous vos guides d'un coup")} className={count ? "primary" : undefined} items={items} />;
}
