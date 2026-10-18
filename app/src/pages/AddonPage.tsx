import { useEffect, useState } from "react";
import { api, type Settings } from "../api";
import { ADDON_GROUPS, ADDON_OPTIONS, addonValue, allAddonValues, requirementMet, type AddonOption } from "../addonSettings";
import { AddonSetup } from "../components/AddonSetup";
import { SettingRow } from "../components/ParamField";
import { Hint } from "../components/Hint";
import { gameLocale, LANGS, useLang, useT } from "../i18n";

export function AddonPage() {
  const t = useT();
  const lang = useLang();
  const [settings, setSettings] = useState<Settings | null>(null);
  // What is saved: a change is saved and the addon installed again a moment after it.
  const [saved, setSaved] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  // Whether the addon in the game has these settings (null: not known yet).
  const [synced, setSynced] = useState<boolean | null>(null);
  const checkSynced = () =>
    api
      .addonSynced()
      .then(setSynced)
      .catch(() => setSynced(null));
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [polestar, setPolestar] = useState(false);
  // The setup questions shown instead of the settings; asked by themselves the first time.
  const [setup, setSetup] = useState(false);

  // Every option with its value (the default when unset), the guide language and what shows the guides.
  const snapshot = (s: Settings) =>
    JSON.stringify([
      ADDON_OPTIONS.map((o) => addonValue(s.addon, o.key)),
      s.guide_locale ?? gameLocale(lang),
      s.guide_addon ?? "factoruide",
    ]);
  useEffect(() => {
    api
      .getSettings()
      .then((s) => {
        setSettings(s);
        setSaved(snapshot(s));
        if (!s.addon_setup_done && !Object.keys(s.addon).length) setSetup(true);
      })
      .catch((e) => setError(String(e)));
    checkSynced();
    api
      .polestarInstalled()
      .then(setPolestar)
      .catch(() => setPolestar(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Save the settings (every option written with its value); the game gets them on demand. */
  const save = async (s: Settings) => {
    setError(null);
    setSaving(true);
    try {
      const addon = allAddonValues(s.addon);
      // What the game was installed with stays the app's (null: kept as it is).
      const next = { ...s, addon, addon_installed: null, guide_locale: s.guide_locale ?? gameLocale(lang), guide_addon: s.guide_addon ?? "factoruide" };
      await api.setSettings(next);
      setSaved(snapshot(next));
      setMessage(null);
      await checkSynced();
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setSaving(false);
    }
  };
  const install = async () => {
    setError(null);
    try {
      setMessage(t("Guides dans l'addon : {n} — faites /reload en jeu", { n: await api.installAddon() }));
      await checkSynced();
    } catch (e) {
      setError(String(e));
    }
  };
  // Saved by itself half a second after the last change.
  useEffect(() => {
    if (!settings || saved === null || snapshot(settings) === saved) return;
    const timer = setTimeout(() => save(settings), 500);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings]);
  if (!settings) return error ? <div className="error">{error}</div> : null;

  const plain = (text: string) => text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
  const matches = (o: AddonOption) =>
    (!o.only || o.only === settings.edition) && (!query.trim() || plain(`${t(o.label)} ${o.help ? t(o.help) : ""}`).includes(plain(query.trim())));
  const update = (key: string, v: boolean | number | string) => {
    setMessage(null);
    setSettings({ ...settings, addon: { ...settings.addon, [key]: v } });
  };
  // Guides in the app language until another one is chosen.
  const guideLocale = settings.guide_locale ?? gameLocale(lang);
  // Polestar shows guides on WoW Forever only.
  const forever = settings.edition === "forever";
  const shownBy = forever ? (settings.guide_addon ?? "factoruide") : "factoruide";
  const withPolestar = shownBy === "polestar";
  const choices = [
    { value: "factoruide" as const, label: t("Addon Factoruide") },
    { value: "polestar" as const, label: "Polestar" },
  ];

  if (setup)
    return (
      <div className="addon-page">
        <div className="page-head">
          <h2>{t("Configurer l'addon")}</h2>
        </div>
        {error && <div className="error">{error}</div>}
        <AddonSetup
          settings={settings}
          defaultLocale={gameLocale(lang)}
          first={!settings.addon_setup_done}
          onCancel={() => {
            setSetup(false);
            if (!settings.addon_setup_done) {
              const next = { ...settings, addon_setup_done: true };
              setSettings(next);
              void save(next);
            }
          }}
          onDone={async (next) => {
            setSetup(false);
            setSettings(next);
            if (await save(next)) await install();
          }}
        />
      </div>
    );

  return (
    <div className="addon-page">
      <div className="page-head">
        <h2>
          {t("Addon")}
          <Hint text={t("Les réglages sont enregistrés à chaque modification ; « Installer dans le jeu » les envoie à l'addon (puis /reload en jeu).")} />
        </h2>
        <span className="muted save-status">{saving ? t("Enregistrement…") : message}</span>
        <button onClick={() => setSetup(true)} title={t("Quelques questions pour régler l'addon d'un coup")}>
          {t("Assistant")}
        </button>
        <button
          className={synced === false ? "primary" : undefined}
          onClick={install}
          disabled={saving}
          title={synced === false ? t("L'addon du jeu n'a pas encore ces réglages.") : undefined}
        >
          {t("Installer dans le jeu")}
        </button>
      </div>
      {error && <div className="error">{error}</div>}
      {forever && (
        <section className="panel guide-addon">
          <div className="field">
            <span>
              {t("Guide en jeu")}
              <Hint text={t("L'addon qui vous guide dans le jeu. Avec Polestar, l'addon Factoruide ne fait plus qu'enregistrer votre personnage et ce que le jeu montre, pour créer vos guides et contribuer.")} />
            </span>
            <div className="segmented" role="radiogroup" aria-label={t("Guide en jeu")}>
              {choices.map((c) => (
                <button
                  key={c.value}
                  role="radio"
                  aria-checked={c.value === shownBy}
                  className={c.value === shownBy ? "active" : ""}
                  onClick={() => {
                    setMessage(null);
                    setSettings({ ...settings, guide_addon: c.value });
                  }}
                >
                  {c.label}
                </button>
              ))}
            </div>
          </div>
          {!polestar && <p className="muted">{t("Polestar n'est pas détecté dans WoW Forever. Vous pouvez quand même le choisir : vos guides sont installés pour Polestar (dossier Factoruide_Polestar des addons du jeu) et s'affichent dès que Polestar est installé.")}</p>}
          <p className="muted">{t("Après un changement ici, installez dans le jeu puis redémarrez WoW : le jeu ne découvre un nouvel addon qu'au démarrage.")}</p>
        </section>
      )}
      {withPolestar ? (
        <div className="addon-grid">
          <section className="panel">
            <h3>{t("Vos guides dans Polestar")}</h3>
            <ul className="polestar-steps">
              <li>{t("Les guides suivis par vos personnages sont installés comme un pack de guides de Polestar, « Factoruide », dans la langue choisie ci-dessous.")}</li>
              <li>{t("Dans Polestar, page « Guides de la communauté », choisissez Factoruide sous « Suggérer les guides de » : Polestar propose alors à chaque personnage le guide de sa race et de sa classe.")}</li>
              <li>{t("Deux personnages de même race et classe qui suivent des guides différents se voient proposer les deux : choisissez-le dans la liste des guides de Polestar.")}</li>
              <li>{t("L'automatisation (quêtes, entraîneurs, vols) et l'affichage se règlent dans les options de Polestar.")}</li>
            </ul>
          </section>
          <section className="panel">
            <h3>{t("Langue")}</h3>
            <SettingRow
              kind="select"
              label="Langue des guides dans l'addon"
              help="Phrases du guide et noms des quêtes, PNJ, objets et zones ; anglais quand une traduction manque. Un guide calculé par une ancienne version de l'application garde sa langue."
              options={LANGS.map((l) => ({ value: l.locale, label: l.label }))}
              value={guideLocale}
              initial={gameLocale(lang)}
              onChange={(v) => {
                setMessage(null);
                setSettings({ ...settings, guide_locale: String(v) });
              }}
            />
          </section>
        </div>
      ) : (
        <>
          <input type="search" className="settings-search addon-search" placeholder={t("Rechercher un réglage…")} value={query} onChange={(e) => setQuery(e.target.value)} />
          <div className="addon-grid">
            {!query.trim() && (
              <section className="panel">
                <h3>{t("Langue")}</h3>
                <SettingRow
                  kind="select"
                  label="Langue de l'addon"
                  help="Textes de l'addon, phrases du guide et noms des quêtes, PNJ, objets et zones ; anglais quand une traduction manque. Un guide calculé par une ancienne version de l'application garde sa langue."
                  options={LANGS.map((l) => ({ value: l.locale, label: l.label }))}
                  value={guideLocale}
                  initial={gameLocale(lang)}
                  onChange={(v) => {
                    setMessage(null);
                    setSettings({ ...settings, guide_locale: String(v) });
                  }}
                />
              </section>
            )}
            {ADDON_GROUPS.map((g) => {
              const sections = g.sections.map((sec) => ({ ...sec, options: sec.options.filter(matches) })).filter((sec) => sec.options.length);
              if (!sections.length) return null;
              return (
                <section className="panel" key={g.title}>
                  <h3>{t(g.title)}</h3>
                  {sections.map((sec) => (
                    <div className="addon-section" key={sec.title ?? ""}>
                      {sec.title && <h4>{t(sec.title)}</h4>}
                      {sec.options.map((o) => {
                        const common = {
                          label: o.label,
                          help: o.help,
                          value: addonValue(settings.addon, o.key),
                          initial: o.default,
                          disabled: !requirementMet(settings.addon, o),
                          onChange: (v: boolean | number | string) => update(o.key, v),
                        };
                        if (o.kind === "bool") return <SettingRow key={o.key} {...common} kind="bool" />;
                        if (o.kind === "select") return <SettingRow key={o.key} {...common} kind="select" options={o.options} />;
                        return <SettingRow key={o.key} {...common} kind="number" min={o.min} max={o.max} step={o.step} {...(o.percent ? { unit: "%", scale: 0.01, min: o.min * 100, max: o.max * 100, step: o.step * 100 } : {})} />;
                      })}
                    </div>
                  ))}
                </section>
              );
            })}
            {query.trim() && !ADDON_OPTIONS.some(matches) && <p className="muted">{t("Aucun réglage trouvé.")}</p>}
          </div>
        </>
      )}
    </div>
  );
}
