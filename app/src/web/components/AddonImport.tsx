import { useEffect, useState } from "react";
import { api, type AddonExport, type Edition } from "../../api";
import { useT } from "../../i18n";

/**
 * Web build: the guides the characters follow go to the game as a text pasted with
 * `/fg import`, made again when the guides changed. On WoW Forever, also as a Polestar guide
 * pack to download.
 */
export function AddonImport({ edition, onClose }: { edition: Edition | null; onClose?: () => void }) {
  const t = useT();
  const [exported, setExported] = useState<AddonExport | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api
      .addonExport()
      .then(setExported)
      .catch((e) => setError(String(e)));
  }, []);
  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setError(t("Le navigateur refuse la copie : sélectionnez le texte et copiez-le (Ctrl+C)."));
    }
  };
  return (
    <section className="panel web-import" aria-live="polite">
      <div className="web-import-head">
        <h3>{t("Envoyer au jeu")}</h3>
        {onClose && (
          <button className="link" onClick={onClose}>
            {t("Fermer")}
          </button>
        )}
      </div>
      {error && <div className="error">{error}</div>}
      {!exported && !error && <p className="muted">{t("Préparation…")}</p>}
      {exported && (
        <>
          <p>{t("Guides dans le texte : {n}.", { n: exported.guides })}</p>
          <ol className="web-steps">
            <li>{t("Copiez le texte ci-dessous.")}</li>
            <li>{t("En jeu, tapez /fg import et collez-le dans la fenêtre (Ctrl+V).")}</li>
            <li>{t("Cliquez sur Importer : l'interface se recharge avec vos guides.")}</li>
          </ol>
          <div className="row">
            <button className="primary" onClick={() => copy(exported.text)}>
              {copied ? t("Copié ✓") : t("Copier le texte")}
            </button>
            <span className="muted">{t("{size} Ko", { size: Math.ceil(exported.text.length / 1024) })}</span>
            {edition === "forever" && (
              <button onClick={() => api.polestarPack().catch((e) => setError(String(e)))} title={t("Les mêmes guides pour Polestar : décompressez le pack dans le dossier Interface\\AddOns de WoW Forever.")}>
                {t("Pack Polestar (.zip)")}
              </button>
            )}
          </div>
          <textarea className="web-import-box" readOnly value={exported.text} onFocus={(e) => e.currentTarget.select()} spellCheck={false} />
        </>
      )}
    </section>
  );
}
