import { useState } from "react";
import { api } from "../../api";
import { useT } from "../../i18n";

/** Where players get the addon (an addon site), when the site is built with one. */
const ADDON_URL: string | undefined = import.meta.env.VITE_FG_ADDON_URL;

/** Web build: how to get the addon in the game (the site cannot write into the game's folder). */
export function AddonInstall() {
  const t = useT();
  const [error, setError] = useState<string | null>(null);
  return (
    <section className="panel web-install">
      <h3>{t("Installer l'addon")}</h3>
      <p>{t("L'addon Factoruide (deux dossiers : Factoruide_Profile et Factoruide_Guide) s'installe une fois, puis se met à jour comme les autres addons.")}</p>
      <div className="row">
        {ADDON_URL && (
          <a className="button primary" href={ADDON_URL} target="_blank" rel="noopener">
            {t("Page de l'addon")}
          </a>
        )}
        <button className={ADDON_URL ? undefined : "primary"} onClick={() => api.addonPackage().catch((e) => setError(String(e)))}>
          {t("Télécharger l'addon (.zip)")}
        </button>
      </div>
      <p className="muted">{t("Le zip se décompresse dans le dossier Interface\\AddOns de la version du jeu (par exemple World of Warcraft\\_classic_era_\\Interface\\AddOns), puis redémarrez le jeu.")}</p>
      {error && <div className="error">{error}</div>}
    </section>
  );
}
