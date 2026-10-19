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
      ["Tout se règle en jeu", "Une seule fenêtre pour tout : /fg."],
      ["Polestar", "Sur WoW Forever, vos guides aussi dans Polestar."],
    ],
  },
];

/** The first page: what the app is for, everything it does, and the ways in. */
export function HomePage({ editionName, onNewGuide, onGuides, onAddon }: Props) {
  const t = useT();
  const [guides, setGuides] = useState<GuideSummary[] | null>(null);
  useEffect(() => {
    api.listGuides().then(setGuides).catch(() => setGuides([]));
  }, []);

  const steps = IS_WEB
    ? [
        { title: t("Installez l'addon"), text: t("Depuis CurseForge, comme n'importe quel addon. Tout se fait dans sa fenêtre en jeu : /fg."), action: { label: t("Page de l'addon"), run: onAddon } },
        { title: t("Calculez votre guide"), text: t("Race, classe, métiers, zones, donjons et façon de jouer, au niveau voulu ou depuis votre personnage (en jeu, /fg puis « Exporter mon personnage », collé dans Nouveau guide) : le guide est calculé dans ce navigateur."), action: { label: t("Nouveau guide"), run: onNewGuide } },
        { title: t("Exportez-le"), text: t("Page Guides, « Exporter » puis Addon : un texte à coller en jeu (/fg puis « Importer un guide… »). Ou « Export global » : tous vos guides en un dossier à décompresser dans les addons du jeu."), action: { label: t("Guides"), run: onGuides } },
        { title: t("Suivez-le en jeu"), text: t("En jeu, /fg, page Guides : choisissez le guide de votre personnage (d'office s'il est le seul de sa race et de sa classe)."), action: { label: t("Page de l'addon"), run: onAddon } },
      ]
    : [
        { title: t("Installez l'addon"), text: t("Depuis CurseForge, comme n'importe quel addon. Tout se fait dans sa fenêtre en jeu : /fg."), action: { label: t("Page de l'addon"), run: onAddon } },
        { title: t("Calculez votre guide"), text: t("Race, classe, métiers, zones, donjons et façon de jouer, au niveau voulu ou depuis un personnage enregistré par l'addon : le guide est calculé sur cet ordinateur."), action: { label: t("Nouveau guide"), run: onNewGuide } },
        { title: t("Exportez-le"), text: t("Page Guides, « Export global » : vos guides écrits dans le jeu (dossier Factoruide_Data). Ou « Exporter » sur un guide : le texte à importer en jeu, un fichier à partager."), action: { label: t("Guides"), run: onGuides } },
        { title: t("Suivez-le en jeu"), text: t("En jeu, /fg, page Guides : choisissez le guide de votre personnage (d'office s'il est le seul de sa race et de sa classe)."), action: { label: t("Page de l'addon"), run: onAddon } },
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
