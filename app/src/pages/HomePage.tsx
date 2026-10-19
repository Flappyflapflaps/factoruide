import { useEffect, useState } from "react";
import { api, type GuideSummary } from "../api";
import { useT } from "../i18n";
import { IS_WEB } from "../web/mode";
import guideShot from "../../../docs/guide.png";
import "./home.css";

interface Props {
  /** The game version chosen in the menu. */
  editionName: string;
  onNewGuide: () => void;
  onCharacters: () => void;
  onGuides: () => void;
  /** Open the addon's page (CurseForge). */
  onAddon: () => void;
}

/** What the app does, by theme: a title and one short sentence each. */
const FEATURES: { title: string; items: [string, string][] }[] = [
  {
    title: "Un guide qui vous ressemble",
    items: [
      ["Votre personnage", "Partez du niveau 1 ou de votre personnage tel qu'il est en jeu : quêtes faites, position, équipement."],
      ["Vos métiers", "Deux métiers et les secondaires, avec la compétence à avoir à chaque niveau."],
      ["Vos zones", "Préférez, imposez ou excluez des zones sur la carte."],
      ["Vos donjons", "Imposez-les, excluez-les, ou laissez le guide garder ceux qui valent le coup."],
      ["Votre façon de jouer", "Rythme, prise de risque, part de farm, vols ou marche : le guide s'adapte."],
      ["En groupe", "Un guide pour un duo ou un groupe, chaque classe avec ses quêtes."],
      ["Vos réputations", "Fixez une réputation à atteindre en fin de guide."],
      ["10 langues", "L'application et les guides dans la langue du jeu."],
    ],
  },
  {
    title: "Calculé, pas écrit à la main",
    items: [
      ["La route la plus rapide", "Toute la montée est simulée : trajets, combats, butin, XP, entraîneurs, équipement."],
      ["Farm sur le chemin", "Les monstres croisés en route comptent, le farm pur reste limité."],
      ["En direct", "Suivez le temps du guide qui baisse pendant le calcul."],
      ["Trois versions du jeu", "WoW Forever, Classic Era / Hardcore et TBC Anniversary."],
    ],
  },
  {
    title: "Dans l'application",
    items: [
      ["La carte du guide", "Toute la route sur la carte, rejouée étape par étape."],
      ["Retouches à la main", "Déplacez, ajoutez ou retirez des étapes : le guide est rejoué aussitôt."],
      ["Versions comparées", "Voyez ce qui change entre deux calculs d'un même guide."],
      ["Partage", "Un fichier .fgguide à envoyer aux autres joueurs."],
    ],
  },
  {
    title: "En jeu, avec l'addon",
    items: [
      ["Étape par étape", "Le guide avance tout seul : quête prise, objectif fait, quête rendue."],
      ["Boussole et flèche", "Toujours la direction de la prochaine étape, vols et bateaux compris."],
      ["Automatisations", "Quêtes acceptées et rendues, entraîneurs, vols, ventes et réparations."],
      ["Suivi du temps", "Votre temps par niveau comparé à celui du guide."],
      ["Tout se règle en jeu", "Une fenêtre de réglages : /fg config."],
      ["Polestar", "Sur WoW Forever, vos guides aussi dans Polestar."],
    ],
  },
];

/** The first page: what the app is for, everything it does, and the ways in. */
export function HomePage({ editionName, onNewGuide, onCharacters, onGuides, onAddon }: Props) {
  const t = useT();
  const [guides, setGuides] = useState<GuideSummary[] | null>(null);
  const [characters, setCharacters] = useState<number | null>(null);
  useEffect(() => {
    api.listGuides().then(setGuides).catch(() => setGuides([]));
    api
      .listCharacters()
      .then((c) => setCharacters(c.length))
      .catch(() => setCharacters(0));
  }, []);

  const steps = IS_WEB
    ? [
        { title: t("Installez l'addon"), text: t("Depuis CurseForge, comme n'importe quel addon. Il se règle en jeu : /fg config."), action: { label: t("Page de l'addon"), run: onAddon } },
        { title: t("Ajoutez votre personnage"), text: t("En jeu, /fg export, puis collez le texte dans la page Personnages : le guide partira de là où il en est. Ou partez d'un niveau 1, sans personnage."), action: { label: t("Personnages"), run: onCharacters } },
        { title: t("Calculez votre guide"), text: t("Race, classe, métiers, zones, donjons et façon de jouer : le guide est calculé dans ce navigateur, en quelques minutes."), action: { label: t("Nouveau guide"), run: onNewGuide } },
        { title: t("Envoyez-le au jeu"), text: t("Page Personnages, « Envoyer au jeu » : /fg import et le texte collé, ou le dossier Factoruide_Data à décompresser dans les addons du jeu."), action: { label: t("Personnages"), run: onCharacters } },
      ]
    : [
        { title: t("Installez l'addon"), text: t("Depuis CurseForge, comme n'importe quel addon. Il se règle en jeu : /fg config."), action: { label: t("Page de l'addon"), run: onAddon } },
        { title: t("Jouez un moment"), text: t("L'addon enregistre vos personnages à chaque déconnexion (ou /fg profile) : ils apparaissent dans la page Personnages."), action: { label: t("Personnages"), run: onCharacters } },
        { title: t("Calculez votre guide"), text: t("Race, classe, métiers, zones, donjons et façon de jouer : le guide est calculé sur cet ordinateur, en quelques minutes."), action: { label: t("Nouveau guide"), run: onNewGuide } },
        { title: t("Suivez-le en jeu"), text: t("Choisissez le guide de chaque personnage : l'application l'écrit pour l'addon (dossier Factoruide_Data), puis /reload en jeu."), action: { label: t("Personnages"), run: onCharacters } },
      ];

  return (
    <div className="home-page">
      <section className="home-hero">
        <div className="home-hero-text">
          <h1>
            <span className="logo">★</span> Factoruide
          </h1>
          <p className="home-tagline">{t("Un guide de pexing calculé pour votre personnage.")}</p>
          <p className="home-intro">
            {t("Les guides habituels suivent une seule route écrite pour tout le monde. Factoruide calcule la vôtre : il simule toute la montée de niveau avec vos choix (trajets, combats, butin, XP des quêtes et des monstres, entraîneurs, métiers, équipement) et cherche la route la plus rapide jusqu'à votre niveau cible. L'addon vous la fait suivre en jeu, étape par étape.")}
          </p>
          <div className="home-actions">
            <button className="primary" onClick={onNewGuide}>
              {t("Nouveau guide")}
            </button>
            <button onClick={onCharacters}>
              {t("Personnages")}
              {characters !== null && <span className="home-count">{characters}</span>}
            </button>
            <button onClick={onGuides}>
              {t("Guides")}
              {guides !== null && <span className="home-count">{guides.length}</span>}
            </button>
          </div>
          <p className="muted home-edition">{t("Version du jeu : {edition} (à changer en haut du menu).", { edition: editionName })}</p>
          {guides && guides.length > 0 && (
            <div className="home-recent">
              <h3>{t("Derniers guides")}</h3>
              <ul>
                {guides.slice(0, 4).map((g) => (
                  <li key={g.id}>
                    <button className="link" onClick={onGuides}>
                      {g.name}
                    </button>{" "}
                    <span className="muted">
                      {t("niveau {from} → {to}", { from: g.versions[0].from_level, to: g.versions[0].to_level })} · {g.versions[0].total_time_text}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
        <img className="home-shot" src={guideShot} alt={t("Un guide sur la carte, avec ses étapes et le personnage étape par étape")} />
      </section>

      <section className="home-steps">
        <h2>{t("Comment ça marche")}</h2>
        <ol>
          {steps.map((s, i) => (
            <li key={i} className="panel">
              <span className="home-step-number">{i + 1}</span>
              <div>
                <h3>{s.title}</h3>
                <p>{s.text}</p>
                <button className="link" onClick={s.action.run}>
                  {s.action.label} →
                </button>
              </div>
            </li>
          ))}
        </ol>
      </section>

      <section className="home-features">
        {FEATURES.map((group) => (
          <div key={group.title} className="home-feature-group">
            <h2>{t(group.title)}</h2>
            <div className="home-feature-grid">
              {group.items.map(([title, text]) => (
                <div key={title} className="home-feature">
                  <h3>{t(title)}</h3>
                  <p>{t(text)}</p>
                </div>
              ))}
            </div>
          </div>
        ))}
      </section>

      <p className="muted home-note">
        {IS_WEB
          ? t("Vos guides et personnages restent dans ce navigateur : rien n'est envoyé ailleurs. Pensez à « Sauvegarder », en bas du menu.")
          : t("Vos guides et personnages restent sur cet ordinateur.")}
      </p>
    </div>
  );
}
