import { useEffect, useState } from "react";
import { api, type Settings } from "../api";
import { SettingRow } from "../components/ParamField";
import { Hint } from "../components/Hint";
import { gameLocale, LANGS, useLang, useT } from "../i18n";

/** `only`: shown for that edition only. */
type Option = { only?: Settings["edition"] } & (
  | { key: string; kind: "bool"; label: string; help?: string; default: boolean }
  | { key: string; kind: "number"; label: string; help?: string; default: number; min: number; max: number; step: number; percent?: boolean }
  | { key: string; kind: "select"; label: string; help?: string; default: string; options: { value: string; label: string }[] }
);

/** Mirror of `FG.defaults` in addon/Factoruide_Guide/Settings.lua. */
const OPTIONS: { title: string; options: Option[] }[] = [
  {
    title: "Guide",
    options: [
      { key: "guideShown", kind: "bool", label: "Afficher le guide", help: "La fenêtre de l'étape en cours et des suivantes.", default: true },
      {
        key: "guideMode",
        kind: "select",
        label: "Mode du guide",
        help: "Complet : une carte avec les tâches en cours, la suite et le talent à placer. Minimal : un bandeau en haut de l'écran, la suite au survol. En jeu, le bouton du guide ou une touche (Factoruide) passe de l'un à l'autre, retenu jusqu'au prochain changement ici.",
        default: "full",
        options: [
          { value: "full", label: "Complet" },
          { value: "minimal", label: "Minimal" },
        ],
      },
      { key: "scale", kind: "number", label: "Taille des fenêtres", help: "Taille de toutes les fenêtres de l'addon (guide, suivi, boussole).", default: 1, min: 0.6, max: 1.6, step: 0.05, percent: true },
      { key: "lockFrames", kind: "bool", label: "Verrouiller les fenêtres", help: "Empêche de déplacer les fenêtres de l'addon par erreur.", default: false },
      { key: "waypoints", kind: "bool", label: "Repère de navigation du jeu", help: "Le point de navigation du jeu vers l'étape en cours, sur la carte et à l'écran (pas sur Classic ; laissé à Shortest Path Forever quand il vous guide).", default: true },
      { key: "xpMarker", kind: "bool", label: "Repère du guide sur la barre d'XP", help: "Un trait jaune sur la barre d'expérience montre l'XP que le guide prévoit à l'étape en cours ; au survol, votre avance ou votre retard.", default: true },
      {
        key: "uiTheme",
        kind: "select",
        label: "Couleurs de l'addon",
        help: "Celles de l'application : sombres, ou parchemin (un fond clair jauni, des textes foncés). Pris en compte après /reload.",
        default: "dark",
        options: [
          { value: "dark", label: "Sombre" },
          { value: "parchemin", label: "Parchemin" },
        ],
      },
      { key: "suiteSkin", kind: "bool", label: "Style de la suite d'interface", help: "Avec ElvUI, EllesmereUI ou SpartanUI : leur style à la place de celui de Factoruide. Pris en compte après /reload.", default: false },
    ],
  },
  {
    title: "Pathing",
    options: [
      { key: "compassShown", kind: "bool", label: "Afficher la boussole", help: "La bande en haut de l'écran : l'étape en cours, bien visible avec sa distance, les autres tâches, les quêtes proches à prendre et les lieux choisis plus bas. Un clic sur un point : le guide pointe dessus. En jeu, faites-la glisser pour la déplacer, Ctrl + molette pour sa taille, tirez ses bords pour sa largeur ; /fg compass la masque ou l'affiche, retenu jusqu'au prochain changement ici.", default: true },
      { key: "arrowShown", kind: "bool", label: "Flèche en plus de la boussole", help: "Une flèche 3D qui tourne avec le personnage : vers le chemin prévu tant qu'il y en a un, sinon vers l'étape, avec la distance. Elle se déplace en la faisant glisser.", default: false },
      {
        key: "spfLink",
        only: "forever",
        kind: "bool",
        label: "Lier Shortest Path Forever",
        help: "Si l'addon Shortest Path Forever est installé, il reçoit l'étape en cours et vous y guide : vols, bateaux, chemin à pied et repère du jeu. Un clic sur un point de la boussole change sa destination.",
        default: true,
      },
      {
        key: "pathMode",
        kind: "select",
        label: "Chemin calculé sur la boussole",
        help: "Le chemin à pied prévu par le guide, autour des falaises et de l'eau : une marque verte sur la boussole montre par où passer, orange quand vous vous en écartez. Automatique : sauf quand Shortest Path Forever vous guide.",
        default: "auto",
        options: [
          { value: "auto", label: "Automatique" },
          { value: "on", label: "Toujours" },
          { value: "off", label: "Jamais" },
        ],
      },
      { key: "pathMap", kind: "bool", label: "Chemin sur la carte et la minicarte", help: "Le chemin vers l'étape en pointillés verts sur la carte du monde et la minicarte, quand la boussole le montre.", default: true },
    ],
  },
  {
    title: "Lieux sur la boussole",
    options: [
      { key: "compassClass", kind: "bool", label: "Maître de votre classe", default: true },
      { key: "compassProfession", kind: "bool", label: "Maîtres de vos métiers", help: "Ceux des métiers appris ou prévus par le guide.", default: true },
      { key: "compassFlight", kind: "bool", label: "Maîtres de vol", default: true },
      { key: "compassInn", kind: "bool", label: "Aubergistes", default: false },
      { key: "compassRepair", kind: "bool", label: "Réparation", default: false },
      { key: "compassFood", kind: "bool", label: "Nourriture et boissons", default: false },
      { key: "compassBank", kind: "bool", label: "Banquiers", default: false },
      { key: "compassAuction", kind: "bool", label: "Hôtels des ventes", default: false },
      { key: "compassStable", kind: "bool", label: "Maîtres des écuries", help: "Chasseurs seulement.", default: false },
      { key: "compassMail", kind: "bool", label: "Boîtes aux lettres", default: false },
      { key: "compassRange", kind: "number", label: "Distance des lieux (mètres)", help: "Les plus proches de chaque sorte à moins de cette distance du personnage.", default: 300, min: 100, max: 1000, step: 50 },
    ],
  },
  {
    title: "Immersion",
    options: [
      {
        key: "windGuide",
        kind: "select",
        label: "Le vent à la place de la boussole",
        help: "Pas de boussole : de temps en temps une rafale passe près du personnage et file vers l'étape en cours, plus souvent quand vous partez ailleurs, et tourbillonne à l'arrivée. Pas en combat ni en instance. Automatique : avec Poulpix UI Immersive.",
        default: "auto",
        options: [
          { value: "auto", label: "Automatique" },
          { value: "on", label: "Toujours" },
          { value: "off", label: "Jamais" },
        ],
      },
      {
        key: "windStyle",
        kind: "select",
        label: "Aspect du vent",
        default: "wind",
        options: [
          { value: "wind", label: "Traînées de vent" },
          { value: "mist", label: "Brume basse" },
          { value: "leaves", label: "Feuilles" },
          { value: "wisps", label: "Feux follets" },
        ],
      },
      { key: "windInterval", kind: "number", label: "Secondes entre deux rafales", help: "Plus souvent quand vous tournez le dos à l'étape, moins quand vous y allez tout droit. Une rafale à la demande : raccourci dans les touches du jeu (Factoruide).", default: 8, min: 3, max: 25, step: 1 },
      { key: "windIntensity", kind: "number", label: "Force du vent", default: 1, min: 0.3, max: 2, step: 0.1, percent: true },
      { key: "poulpixFade", kind: "bool", label: "Fondu avec Poulpix UI Immersive", help: "Les fenêtres de Factoruide s'effacent avec l'interface de Poulpix pendant l'exploration, et reviennent avec elle ou sous la souris.", default: true },
    ],
  },
  {
    title: "Suivi des quêtes",
    options: [
      { key: "trackerShown", kind: "bool", label: "Afficher le suivi des quêtes Factoruide", help: "Une ligne par quête dans l'ordre du guide (objectifs au survol), et des onglets pour les métiers, la réputation et les objets à garder pour les quêtes (sacs et banque).", default: true },
      { key: "replaceTracker", kind: "bool", label: "Masquer le suivi de quêtes de Blizzard", help: "Le suivi de Blizzard est caché tant que celui de Factoruide est affiché.", default: true },
      { key: "trackerQuests", kind: "number", label: "Quêtes affichées", help: "Les prochaines quêtes du journal à faire, dans l'ordre du guide.", default: 8, min: 1, max: 35, step: 1 },
      { key: "trackerAll", kind: "bool", label: "Afficher tout le journal", help: "Sinon, seulement les quêtes du journal que le guide fait encore.", default: false },
      { key: "professionsShown", kind: "bool", label: "Afficher la progression des métiers", help: "Compétence actuelle face au palier prévu pour votre niveau.", default: true },
    ],
  },
  {
    title: "Quêtes",
    options: [
      { key: "autoAccept", kind: "bool", label: "Accepter les quêtes automatiquement", help: "Maintenir Maj en parlant au PNJ pour désactiver.", default: true },
      { key: "routeQuestsOnly", kind: "bool", label: "Seulement les quêtes du guide", help: "Celles que le guide prend encore : ni celles qu'il abandonne, ni celles marquées faites ou laissées derrière, ni les escortes partagées par le groupe. Vous pouvez toujours les prendre à la main.", default: true },
      { key: "autoTurnIn", kind: "bool", label: "Rendre les quêtes automatiquement", help: "Rend les quêtes terminées dès qu'on parle au PNJ.", default: true },
      { key: "autoReward", kind: "bool", label: "Choisir la récompense (meilleur prix de vente)", help: "Celle prévue par le guide d'abord, sinon la meilleure amélioration d'équipement.", default: false },
    ],
  },
  {
    title: "Macro de ciblage",
    options: [
      { key: "targetMacro", kind: "bool", label: "Mettre à jour la macro « FG Cible »", help: "Elle cible les mobs ou le PNJ de l'étape en cours. Tapez /fg macro en jeu, posez-la sur une barre et associez-lui un raccourci.", default: true },
      { key: "targetMark", kind: "bool", label: "Marquer la cible d'un crâne", help: "Pose un crâne sur la cible de la macro pour la repérer (WoW Classic et TBC ; WoW Forever l'interdit aux macros).", default: true },
    ],
  },
  {
    title: "Divers",
    options: [
      { key: "fastLoot", kind: "bool", label: "Loot rapide", help: "Quand le loot automatique du jeu est actif.", default: true },
      { key: "autoTrain", kind: "bool", label: "Apprendre les sorts chez l'entraîneur de classe", help: "Achète les sorts à l'ouverture de la fenêtre de l'entraîneur.", default: true },
      { key: "trainUseful", kind: "bool", label: "Seulement les sorts utiles au leveling", help: "Ceux que le guide compte pour la puissance du personnage ; les autres restent à acheter soi-même.", default: true },
      { key: "autoFly", kind: "bool", label: "Prendre le vol de l'étape automatiquement", help: "À l'ouverture de la carte des vols (Maj pour choisir soi-même).", default: true },
      { key: "questItemButton", kind: "bool", label: "Bouton de l'objet de quête", help: "À côté du guide (près des barres d'action en mode minimal) ; raccourci dans les touches du jeu (Factoruide).", default: true },
      { key: "mapPins", kind: "bool", label: "Prochaines étapes sur la carte du monde", help: "Marque les prochaines étapes du guide sur la carte du monde.", default: true },
      { key: "mapPinsCount", kind: "number", label: "Étapes affichées sur la carte", help: "Nombre d'étapes à venir marquées sur la carte du monde.", default: 15, min: 3, max: 40, step: 1 },
      { key: "levelTiming", kind: "bool", label: "Temps de jeu comparé au guide", help: "Avance ou retard sur le temps prévu, dans l'en-tête du guide ; /fg time pour le détail.", default: true },
    ],
  },
  {
    title: "Marchands",
    options: [
      { key: "autoSellJunk", kind: "bool", label: "Vendre les objets gris", help: "Maintenir Maj en ouvrant le marchand pour désactiver.", default: true },
      { key: "autoRepair", kind: "bool", label: "Réparer automatiquement", help: "Répare l'équipement chez les marchands qui le proposent.", default: true },
      { key: "autoSupplies", kind: "bool", label: "Acheter nourriture, eau et munitions", help: "Les meilleures de votre niveau chez le marchand, dans la limite de 30 % de votre or.", default: false },
      { key: "suppliesFood", kind: "number", label: "Nourriture à garder", help: "Quantité de nourriture que l'achat automatique garde dans les sacs.", default: 20, min: 0, max: 100, step: 5 },
      { key: "suppliesWater", kind: "number", label: "Eau à garder", help: "Classes à mana seulement.", default: 20, min: 0, max: 100, step: 5 },
      { key: "suppliesAmmo", kind: "number", label: "Munitions à garder", help: "Chasseurs : flèches ou balles selon l'arme.", default: 1000, min: 0, max: 3000, step: 200 },
    ],
  },
  {
    title: "Équipement et talents",
    options: [
      { key: "gearAdvisor", kind: "bool", label: "Conseiller d'équipement", help: "Amélioration indiquée dans les infobulles et sur les récompenses ; le choix automatique de récompense la prend en premier.", default: true },
      { key: "talentAdvisor", kind: "bool", label: "Talent conseillé", help: "Plan de leveling de la classe, affiché dans le guide quand il reste des points.", default: true },
      { key: "autoLearnTalents", kind: "bool", label: "Apprendre les talents automatiquement", help: "Dépense les points de talent selon le plan de leveling de la classe.", default: false },
    ],
  },
  {
    title: "Groupe et sécurité",
    options: [
      { key: "partySync", kind: "bool", label: "Partager la progression avec le groupe", help: "Les membres avec Factoruide voient vos objectifs à côté des leurs.", default: true },
      { key: "safetyAlerts", kind: "bool", label: "Alertes de danger", help: "Cible élite ou bien plus haute que vous, vie basse ; objectifs élites signalés dans le guide.", default: true },
    ],
  },
];

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

  // Every option with its value (the default when unset), the guide language and what shows the guides.
  const snapshot = (s: Settings) =>
    JSON.stringify([
      OPTIONS.flatMap((g) => g.options.map((o) => s.addon[o.key] ?? o.default)),
      s.guide_locale ?? gameLocale(lang),
      s.guide_addon ?? "factoruide",
    ]);
  useEffect(() => {
    api
      .getSettings()
      .then((s) => {
        setSettings(s);
        setSaved(snapshot(s));
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
      const addon: Record<string, boolean | number | string> = {};
      for (const g of OPTIONS) for (const o of g.options) addon[o.key] = s.addon[o.key] ?? o.default;
      const next = { ...s, addon, guide_locale: s.guide_locale ?? gameLocale(lang), guide_addon: s.guide_addon ?? "factoruide" };
      await api.setSettings(next);
      setSaved(snapshot(next));
      setMessage(null);
      await checkSynced();
    } catch (e) {
      setError(String(e));
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

  const value = (o: Option) => settings.addon[o.key] ?? o.default;
  const plain = (text: string) => text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
  const matches = (o: Option) =>
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

  return (
    <div className="addon-page">
      <div className="page-head">
        <h2>
          {t("Addon")}
          <Hint text={t("Les réglages sont enregistrés à chaque modification ; « Installer dans le jeu » les envoie à l'addon (puis /reload en jeu).")} />
        </h2>
        <span className="muted save-status">{saving ? t("Enregistrement…") : message}</span>
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
            {OPTIONS.map((g) => {
              const options = g.options.filter(matches);
              if (!options.length) return null;
              return (
                <section className="panel" key={g.title}>
                  <h3>{t(g.title)}</h3>
                  {options.map((o) => {
                    const common = { label: o.label, help: o.help, value: value(o), initial: o.default, onChange: (v: boolean | number | string) => update(o.key, v) };
                    if (o.kind === "bool") return <SettingRow key={o.key} {...common} kind="bool" />;
                    if (o.kind === "select") return <SettingRow key={o.key} {...common} kind="select" options={o.options} />;
                    return <SettingRow key={o.key} {...common} kind="number" min={o.min} max={o.max} step={o.step} {...(o.percent ? { unit: "%", scale: 0.01, min: o.min * 100, max: o.max * 100, step: o.step * 100 } : {})} />;
                  })}
                </section>
              );
            })}
            {query.trim() && !OPTIONS.some((g) => g.options.some(matches)) && <p className="muted">{t("Aucun réglage trouvé.")}</p>}
          </div>
        </>
      )}
    </div>
  );
}
