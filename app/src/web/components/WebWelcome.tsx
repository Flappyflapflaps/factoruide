import { useState } from "react";
import { api, type Edition, type EditionInfo } from "../../api";
import { useT } from "../../i18n";
import { AddonInstall } from "./AddonInstall";

interface Props {
  editions: EditionInfo[];
  /** Switch the game version the app works on. */
  onEdition: (edition: Edition) => Promise<void>;
  /** `characters`: go on to the Characters page (to paste a character). */
  onDone: (characters: boolean) => void;
}

/**
 * Web build, first visit: which game version, the addon to install, then how a character
 * comes from the game. Everything stays in this browser.
 */
export function WebWelcome({ editions, onEdition, onDone }: Props) {
  const t = useT();
  const [step, setStep] = useState(0);
  const [edition, setEdition] = useState<Edition | null>(null);
  const [error, setError] = useState<string | null>(null);
  const finish = async (characters: boolean) => {
    const settings = await api.getSettings().catch(() => null);
    if (settings) await api.setSettings({ ...settings, welcome_done: true }).catch(() => {});
    onDone(characters);
  };
  const next = async () => {
    setError(null);
    try {
      if (step === 0 && edition) await onEdition(edition);
      setStep(step + 1);
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <div className="welcome-overlay">
      <div className="panel welcome" role="dialog" aria-modal="true" aria-labelledby="welcome-title">
        <h2 id="welcome-title">{t("Bienvenue dans Factoruide")}</h2>
        {step === 0 && (
          <>
            <p>{t("Sur quelle version du jeu jouez-vous ? Vous pourrez en changer à tout moment en haut du menu.")}</p>
            <div className="setup-choices" role="radiogroup" aria-label={t("Version du jeu")}>
              {editions
                .filter((e) => e.available)
                .map((e) => (
                  <button key={e.key} role="radio" aria-checked={edition === e.key} className={`setup-choice${edition === e.key ? " active" : ""}`} onClick={() => setEdition(e.key)}>
                    <strong>{e.name}</strong>
                    <small>{t("Niveau maximum {n}", { n: e.max_level })}</small>
                  </button>
                ))}
            </div>
          </>
        )}
        {step === 1 && <AddonInstall />}
        {step === 2 && (
          <>
            <p>{t("Factoruide calcule vos guides dans ce navigateur, et les garde ici (rien n'est envoyé ailleurs).")}</p>
            <ol className="web-steps">
              <li>{t("En jeu, tapez /fg export avec un personnage, et collez le texte dans la page Personnages : le guide partira de là où il en est.")}</li>
              <li>{t("Créez un guide (ou partez d'un niveau 1 sans personnage).")}</li>
              <li>{t("Page Personnages, « Envoyer au jeu » : copiez le texte, puis /fg import en jeu.")}</li>
            </ol>
          </>
        )}
        {error && <div className="error">{error}</div>}
        <div className="setup-actions">
          <button className="link" onClick={() => finish(false)}>
            {step < 2 ? t("Passer") : t("Plus tard")}
          </button>
          <span />
          {step < 2 ? (
            <button className="primary" disabled={step === 0 && !edition} onClick={next}>
              {t("Suivant")}
            </button>
          ) : (
            <button className="primary" onClick={() => finish(true)}>
              {t("Ajouter un personnage")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
