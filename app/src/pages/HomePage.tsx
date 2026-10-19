import { useEffect, useState } from "react";
import { api, type GuideSummary } from "../api";
import { useT } from "../i18n";
import { IS_WEB } from "../web/mode";
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

/** The first page: what the app is for, how it works, and the ways in. */
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
        <p className="muted home-edition">{t("Version du jeu : {edition} (à changer en haut du menu). WoW Forever, Classic Era / Hardcore et TBC Anniversary.", { edition: editionName })}</p>
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

      {guides && guides.length > 0 && (
        <section className="home-recent">
          <h2>{t("Derniers guides")}</h2>
          <ul>
            {guides.slice(0, 5).map((g) => (
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
        </section>
      )}

      <p className="muted home-note">
        {IS_WEB
          ? t("Vos guides et personnages restent dans ce navigateur : rien n'est envoyé ailleurs. Pensez à « Sauvegarder », en bas du menu.")
          : t("Vos guides et personnages restent sur cet ordinateur.")}
      </p>
    </div>
  );
}
