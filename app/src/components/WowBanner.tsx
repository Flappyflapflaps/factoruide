import { useCallback, useEffect, useState } from "react";
import { api, SETTINGS_CHANGED, type AddonHealth, type WowStatus } from "../api";
import { useT } from "../i18n";
import { CLIENTS, SUPPORTED } from "../wow";
import { IS_WEB } from "../web/mode";

interface Props {
  /** The game version the app works on (switching it fires SETTINGS_CHANGED). */
  editionName: string;
  onSettings: () => void;
  onAddon: () => void;
}

/**
 * Over every page: a red bar while the guides cannot go into the game (no World of Warcraft
 * folder, no game in it, not the game version the app works on, no right to write there), the
 * addon is not installed or the game does not load it; an orange one while the addon is older
 * than the app needs or the game must restart. Gone once it is right.
 */
export function WowBanner({ editionName, onSettings, onAddon }: Props) {
  const t = useT();
  const [status, setStatus] = useState<WowStatus | null>(null);
  const [health, setHealth] = useState<AddonHealth | null>(null);
  const check = useCallback(() => {
    api
      .wowStatus()
      .then(async (s) => {
        setStatus(s);
        setHealth(s.problem ? null : await api.addonHealth());
      })
      .catch(() => setStatus(null));
  }, []);
  useEffect(() => {
    check();
    // Saved settings or game version, guides written, a game installed or played meanwhile.
    window.addEventListener(SETTINGS_CHANGED, check);
    window.addEventListener("focus", check);
    return () => {
      window.removeEventListener(SETTINGS_CHANGED, check);
      window.removeEventListener("focus", check);
    };
  }, [check]);
  if (status?.problem) {
    const dir = status.root;
    const others = status.clients.filter((c) => SUPPORTED.includes(c)).map((c) => CLIENTS[c] ?? c);
    const text =
      status.problem === "missing"
        ? t("Dossier World of Warcraft introuvable : {dir}.", { dir })
        : status.problem === "no_game"
          ? t("Aucune version du jeu dans {dir} : choisissez le dossier World of Warcraft, celui qui contient _classic_era_, _anniversary_ ou _classic_beta_.", { dir })
          : status.problem === "not_writable"
            ? t("L'application n'a pas le droit d'écrire dans {dir} : donnez à votre utilisateur Windows le droit de modifier le dossier World of Warcraft (Propriétés, onglet Sécurité), ou lancez l'application une fois en administrateur.", { dir })
            : t("{edition} n'est pas installé dans {dir}.", { edition: editionName, dir }) + (others.length ? " " + t("Versions trouvées : {list} (à choisir en haut du menu).", { list: others.join(", ") }) : "");
    return (
      <div className="wow-banner" role="alert">
        <span className="wow-banner-text">
          <strong>{t("L'addon ne peut pas être installé dans le jeu.")}</strong> {text}
        </span>
        <button onClick={onSettings}>{t("Choisir le dossier")}</button>
      </div>
    );
  }
  if (health?.not_loaded)
    return (
      <div className="wow-banner" role="alert">
        <span className="wow-banner-text">
          <strong>{t("Le jeu n'a pas chargé l'addon.")}</strong> {t("Le jeu a tourné depuis l'installation sans le lancer : à l'écran de sélection du personnage, bouton AddOns, cochez Factoruide (et « Charger les AddOns périmés » si on vous le demande).")}
        </span>
        <button onClick={onAddon}>{t("Page Addon")}</button>
      </div>
    );
  // In a browser: the addon version of the last export pasted.
  if (IS_WEB && health?.installed_version && health.installed_version !== health.app_version)
    return (
      <div className="wow-banner outdated" role="status">
        <span className="wow-banner-text">
          <strong>{t("L'addon du jeu n'est pas à jour.")}</strong> {t("Le dernier export vient de la version {old} de l'addon : mettez-le à jour depuis CurseForge pour avoir les nouveautés.", { old: health.installed_version })}
        </span>
        <button onClick={onAddon}>{t("Page de l'addon")}</button>
      </div>
    );
  // The desktop app: the addon comes from CurseForge, the app writes the guides beside it.
  if (!IS_WEB && health && !health.installed_version)
    return (
      <div className="wow-banner" role="alert">
        <span className="wow-banner-text">
          <strong>{t("L'addon Factoruide n'est pas installé dans {edition}.", { edition: editionName })}</strong> {t("Installez-le depuis CurseForge (ou votre gestionnaire d'addons) : il enregistre vos personnages et affiche vos guides en jeu.")}
        </span>
        <button className="primary" onClick={onAddon}>
          {t("Page de l'addon")}
        </button>
      </div>
    );
  if (!IS_WEB && health?.outdated)
    return (
      <div className="wow-banner outdated" role="status">
        <span className="wow-banner-text">
          <strong>{t("L'addon du jeu n'est pas à jour.")}</strong> {t("Il est en version {old}, l'application demande la {version} ou plus récente : mettez-le à jour depuis CurseForge.", { old: health.installed_version ?? "?", version: health.required_version ?? "?" })}
        </span>
        <button className="primary" onClick={onAddon}>
          {t("Page de l'addon")}
        </button>
      </div>
    );
  if (!IS_WEB && health?.restart_needed)
    return (
      <div className="wow-banner outdated" role="status">
        <span className="wow-banner-text">
          <strong>{t("Redémarrez le jeu s'il est ouvert.")}</strong> {t("Vos guides sont dans un nouveau dossier d'addon (Factoruide_Data), que le jeu ne découvre qu'au démarrage : un /reload ne suffit pas cette fois.")}
        </span>
      </div>
    );
  return null;
}
