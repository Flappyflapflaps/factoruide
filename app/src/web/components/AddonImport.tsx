import { useEffect, useState } from "react";
import { api, type AddonExport } from "../../api";
import { useT } from "../../i18n";
import { download, fromBase64 } from "../mode";

/**
 * Web build: the guides and the addon's settings go to the game as a text pasted with
 * `/fg import` (or, for Polestar, a pack to unzip in the game's AddOns folder). Made again when
 * the guides or the settings changed.
 */
export function AddonImport({ onClose }: { onClose?: () => void }) {
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
      {exported?.kind === "text" && (
        <>
          <p>{t("Guides dans le texte : {n}, avec les réglages de l'addon.", { n: exported.guides })}</p>
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
          </div>
          <textarea className="web-import-box" readOnly value={exported.text} onFocus={(e) => e.currentTarget.select()} spellCheck={false} />
        </>
      )}
      {exported?.kind === "zip" && (
        <>
          <p>{t("Guides dans le pack Polestar : {n}.", { n: exported.guides })}</p>
          <ol className="web-steps">
            <li>{t("Téléchargez le pack.")}</li>
            <li>{t("Décompressez-le dans le dossier Interface\\AddOns de WoW Forever (il contient le dossier Factoruide_Polestar, qui remplace l'ancien).")}</li>
            <li>{t("Redémarrez le jeu s'il était ouvert.")}</li>
          </ol>
          <button className="primary" onClick={() => download(exported.file, fromBase64(exported.base64), "application/zip")}>
            {t("Télécharger le pack")}
          </button>
        </>
      )}
    </section>
  );
}
