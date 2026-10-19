import { useCallback, useEffect, useState } from "react";
import { api, SETTINGS_CHANGED, type AddonHealth } from "../api";
import { useDate, useT } from "../i18n";
import { IS_WEB } from "../web/mode";

/** Looked at again on these, and when the window comes back (a game played meanwhile). */
export function useAddonHealth() {
  const [health, setHealth] = useState<AddonHealth | null>(null);
  const check = useCallback(() => {
    api
      .addonHealth()
      .then(setHealth)
      .catch(() => setHealth(null));
  }, []);
  useEffect(() => {
    check();
    window.addEventListener(SETTINGS_CHANGED, check);
    window.addEventListener("focus", check);
    return () => {
      window.removeEventListener(SETTINGS_CHANGED, check);
      window.removeEventListener("focus", check);
    };
  }, [check]);
  return health;
}

/** What the game did with the addon: installed by which version, last seen in game, turned off. */
export function AddonInGame() {
  const t = useT();
  const date = useDate();
  const health = useAddonHealth();
  if (!health) return null;
  const lines: { ok: boolean; text: string }[] = [];
  // In a browser, the game is known from the characters' exports pasted.
  if (IS_WEB) {
    if (!health.seen) lines.push({ ok: false, text: t("Aucun export reçu de ce jeu : tapez /fg export en jeu, et collez le texte dans la page Personnages.") });
    else {
      lines.push({ ok: true, text: t("Dernier export reçu le {date} (addon {version}).", { date: date(health.seen.time), version: health.seen.version || "?" }) });
      if (health.seen.version && health.seen.version !== health.app_version)
        lines.push({ ok: false, text: t("L'addon du jeu est en version {old}, l'application en {version} : mettez l'addon à jour.", { old: health.seen.version, version: health.app_version }) });
    }
  } else if (!health.installed_version) lines.push({ ok: false, text: t("Pas encore installé dans le jeu.") });
  else {
    lines.push({
      ok: health.complete,
      text: health.complete
        ? t("Installé par la version {version} de l'application, le {date}.", { version: health.installed_version, date: health.installed_at ? date(health.installed_at) : "?" })
        : t("Des fichiers de l'addon manquent : installez-le à nouveau."),
    });
    lines.push(
      health.seen
        ? { ok: !health.not_loaded, text: t("Vu en jeu le {date} (version {version}).", { date: date(health.seen.time), version: health.seen.version }) }
        : { ok: false, text: t("Pas encore vu en jeu : connectez un personnage, puis faites /reload ou déconnectez-vous (le jeu n'enregistre les données des addons qu'à ce moment-là).") },
    );
  }
  if (health.not_loaded)
    lines.push({ ok: false, text: t("Le jeu a tourné depuis l'installation sans lancer l'addon : à l'écran de sélection du personnage, bouton AddOns, cochez Factoruide (et « Charger les AddOns périmés » si on vous le demande).") });
  if (health.disabled_for.length)
    lines.push({ ok: false, text: t("Désactivé dans la liste des AddOns du jeu pour : {list}.", { list: health.disabled_for.join(", ") }) });
  if (health.restart_needed)
    lines.push({ ok: false, text: t("La dernière installation ajoute des fichiers : si le jeu est ouvert, quittez-le et relancez-le (un /reload ne suffit pas).") });
  return (
    <section className="panel addon-in-game">
      <h3>{t("Dans le jeu")}</h3>
      <ul>
        {lines.map((l) => (
          <li key={l.text} className={l.ok ? "ok" : "warn"}>
            <span aria-hidden>{l.ok ? "✓" : "!"}</span> {l.text}
          </li>
        ))}
      </ul>
    </section>
  );
}
