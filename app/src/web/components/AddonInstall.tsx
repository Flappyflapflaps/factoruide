import { useT } from "../../i18n";

/** The addon's page on the addon site (set at build time), else a search there. */
export const ADDON_URL: string = import.meta.env.VITE_FG_ADDON_URL || "https://www.curseforge.com/wow/search?search=factoruide";

/** Where the addon comes from: an addon site (CurseForge), installed and updated like any addon. */
export function AddonInstall() {
  const t = useT();
  return (
    <section className="panel web-install">
      <h3>{t("Installer l'addon")}</h3>
      <p>{t("Installez l'addon Factoruide depuis CurseForge (ou votre gestionnaire d'addons), comme n'importe quel addon : il se met à jour de la même façon. Tout se fait dans sa fenêtre en jeu : /fg.")}</p>
      <div className="row">
        <a className="button primary" href={ADDON_URL} target="_blank" rel="noopener">
          {t("Page de l'addon")}
        </a>
      </div>
    </section>
  );
}
