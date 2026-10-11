import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type Settings } from "../api";
import { LANGS, useLang, useT, type Lang } from "../i18n";

/** Game version folders the app has guides for. */
const SUPPORTED = ["_classic_beta_", "_classic_era_", "_anniversary_"];

/** Known game version folders. */
const CLIENTS: Record<string, string> = {
  _classic_beta_: "WoW Forever (beta)",
  _classic_era_: "Classic Era",
  _anniversary_: "TBC Anniversary",
  _classic_: "Mists of Pandaria Classic",
  _retail_: "Retail",
  _classic_era_ptr_: "Classic Era PTR",
  _classic_ptr_: "Classic PTR",
  _ptr_: "PTR",
  _beta_: "Beta",
};

interface Props {
  /** Applied at once, saved with the other settings. */
  onLanguage: (lang: Lang) => void;
}

export function SettingsPage({ onLanguage }: Props) {
  const t = useT();
  const lang = useLang();
  const [settings, setSettings] = useState<Settings | null>(null);
  // Game version folders in the WoW folder (null: not looked yet).
  const [found, setFound] = useState<string[] | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.getSettings().then(setSettings).catch((e) => setError(String(e)));
  }, []);
  const wowDir = settings?.wow_dir;
  useEffect(() => {
    if (wowDir != null) api.listClients(wowDir).then(setFound).catch(() => setFound([]));
  }, [wowDir]);
  if (!settings) return error ? <div className="error">{error}</div> : null;


  return (
    <div className="settings">
      <div className="page-head">
        <h2>{t("Réglages")}</h2>
      </div>
    <section className="panel">
      <label className="field">
        <span>{t("Langue de l'application")}</span>
        <select
          value={settings.language ?? lang}
          onChange={(e) => {
            const next = { ...settings, language: e.target.value };
            setSettings(next);
            onLanguage(e.target.value as Lang);
            api.setSettings(next).catch((err) => setError(String(err)));
          }}
        >
          {LANGS.map((l) => (
            <option key={l.key} value={l.key}>
              {l.label}
            </option>
          ))}
        </select>
        <small>{t("Aussi celle des guides affichés dans l'application.")}</small>
      </label>
      <div className={`field${found && !found.some((c) => SUPPORTED.includes(c)) ? " invalid" : ""}`}>
        <label htmlFor="wow-dir">{t("Dossier World of Warcraft")}</label>
        <div className="row">
          <input id="wow-dir" value={settings.wow_dir} onChange={(e) => setSettings({ ...settings, wow_dir: e.target.value })} />
          <button
            onClick={async () => {
              const dir = await open({ directory: true, defaultPath: settings.wow_dir || undefined, title: t("Dossier World of Warcraft") }).catch(() => null);
              if (typeof dir === "string") setSettings({ ...settings, wow_dir: dir });
            }}
          >
            {t("Parcourir…")}
          </button>
        </div>
        <small>{t("Celui qui contient .build.info.")}</small>
      </div>
      <div className="field">
        <span>{t("Versions du jeu trouvées")}</span>
        {found === null ? null : found.length === 0 ? (
          <small className="field-error">{t("Aucune version trouvée dans ce dossier.")}</small>
        ) : (
          <ul className="clients">
            {found.map((c) => (
              <li key={c} className={SUPPORTED.includes(c) ? "ok" : "unsupported"}>
                {SUPPORTED.includes(c) ? "✓" : "–"} {CLIENTS[c] ?? c}
                {!SUPPORTED.includes(c) && <small> {t("(non prise en charge)")}</small>}
              </li>
            ))}
          </ul>
        )}
        <small>{t("L'addon s'installe dans la version choisie dans le menu.")}</small>
      </div>
      <details className="advanced-settings">
        <summary>{t("Avancé")}</summary>
        <label className="field">
          <span>{t("Serveur de contribution")}</span>
          <input
            placeholder="https://…"
            value={settings.contribution_url ?? ""}
            onChange={(e) => setSettings({ ...settings, contribution_url: e.target.value })}
          />
          <small>{t("Adresse qui reçoit les données des joueurs (page Contribuer).")}</small>
        </label>
      </details>
      <button
        className="primary"
        onClick={async () => {
          try {
            await api.setSettings(settings);
            setMessage(t("Réglages enregistrés"));
          } catch (e) {
            setError(String(e));
          }
        }}
      >
        {t("Enregistrer")}
      </button>
      {message && <div className="success">{message}</div>}
      {error && <div className="error">{error}</div>}
    </section>
    </div>
  );
}
