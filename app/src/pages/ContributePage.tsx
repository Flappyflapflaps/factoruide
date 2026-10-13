import type { Edition } from "../api";
import { ContributePanel } from "../components/ContributePanel";
import { Link } from "../components/Markdown";
import { useT } from "../i18n";

interface Props {
  /** Game version shown in the menu (the tools differ by version). */
  edition: Edition | null;
}

/** Wowhead's tool page for a game version. */
const wowheadClient = (edition: Edition | null) => (edition === "forever" ? "https://www.wowhead.com/forever/client" : "https://www.wowhead.com/classic/client");

/**
 * Ways to send what one's game sees to the databases the guides are built from: Factoruide's own
 * server (not open yet), Questie's and Wowhead's tools. Each improves every community tool.
 */
export function ContributePage({ edition }: Props) {
  const t = useT();
  return (
    <div className="contribute-page">
      <div className="page-head">
        <h2>{t("Contribuer")}</h2>
      </div>
      <p className="muted contribute-intro">
        {t("Les guides sont calculés à partir de bases de quêtes construites par la communauté. Ce que votre jeu voit (quêtes, PNJ, objets, positions) les complète et les corrige : chacun de ces outils améliore toutes les bases, et donc tous les outils de la communauté.")}
      </p>
      <div className="contribute-grid">
        <ContributePanel key={edition ?? ""} available={false} />
        <section className="panel tool">
          <div>
            <h3>QuestieTrace</h3>
            <p className="muted">{t("L'outil de l'équipe Questie : un addon qui enregistre votre session de jeu (événements et réponses du jeu, sans nom de joueur ni de royaume) pour corriger et compléter la base de Questie.")}</p>
          </div>
          <ol className="contribute-steps">
            <li>{t("Installez l'addon QuestieTrace.")}</li>
            <li>{t("Jouez normalement.")}</li>
            <li>
              {t("Tapez /qlt export en jeu, copiez le texte et collez-le sur le site de Questie.")}
            </li>
          </ol>
          <ul className="about-links">
            <li>
              <Link url="https://github.com/Questie/QuestieTrace" label={t("Télécharger QuestieTrace")} />
            </li>
            <li>
              <Link url="https://questie.dev/trace" label={t("Envoyer une session (questie.dev/trace)")} />
            </li>
          </ul>
        </section>
        <section className="panel tool">
          <div>
            <h3>Wowhead Client</h3>
            <p className="muted">{t("L'application de Wowhead : elle installe l'addon Wowhead Looter, qui note en jeu les PNJ, objets, butins et quêtes, puis envoie ces données à Wowhead, automatiquement après la fermeture du jeu.")}</p>
          </div>
          <ol className="contribute-steps">
            <li>{t("Installez Wowhead Client et connectez-vous à votre compte Wowhead.")}</li>
            <li>{t("Jouez normalement : l'addon Wowhead Looter collecte en arrière-plan.")}</li>
            <li>{t("Fermez le jeu : le client envoie les données tout seul.")}</li>
          </ol>
          <ul className="about-links">
            <li>
              <Link url={wowheadClient(edition)} label={t("Télécharger Wowhead Client")} />
            </li>
          </ul>
        </section>
      </div>
    </div>
  );
}
