import { useEffect, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { Link, REPO } from "./Markdown";
import { CHECK_FOR_UPDATES } from "./UpdateBanner";
import { api } from "../api";
import { useT } from "../i18n";

/** The project: its version, where its code is, its licenses and where its data comes from. */
export function AboutPanels({ version }: { version: string | null }) {
  const t = useT();
  // Published builds only: a local build has no update channel.
  const [updates, setUpdates] = useState(false);
  useEffect(() => {
    api
      .updatesEnabled()
      .then(setUpdates)
      .catch(() => {});
  }, []);
  return (
    <div className="about">
      <h3 className="about-title">{t("À propos")}</h3>
      <div className="about-grid">
        <section className="panel">
          <h3>Factoruide {version}</h3>
          {updates && <button onClick={() => void emit(CHECK_FOR_UPDATES)}>{t("Rechercher des mises à jour")}</button>}
          <p className="muted">{t("Guides de leveling calculés pour votre personnage, et l'addon qui vous guide en jeu.")}</p>
          <ul className="about-links">
            <li>
              <Link url={REPO} label={t("Code source")} />
            </li>
            <li>
              <Link url={`${REPO}issues`} label={t("Signaler un problème")} />
            </li>
            <li>
              <Link url={`${REPO}releases`} label={t("Toutes les versions")} />
            </li>
          </ul>
        </section>
        <section className="panel">
          <h3>{t("Licences")}</h3>
          <ul className="about-links">
            <li>
              {t("Code de l'application et de l'addon :")} <Link url="LICENSE" label="MIT" />
            </li>
            <li>
              {t("Bases de quêtes :")} <Link url="data/LICENSE" label="GPL-2.0-or-later" />
            </li>
          </ul>
        </section>
        <section className="panel">
          <h3>{t("Données")}</h3>
          <p className="muted">
            {t("Les bases de quêtes sont compilées à partir du client du jeu, de bases de données communautaires et des contributions des joueurs. Chaque source et sa licence sont détaillées dans la notice.")}
          </p>
          <p>
            <Link url="NOTICE.md" label={t("Lire la notice sur les données")} />
          </p>
          <p className="muted">
            {t("World of Warcraft et Blizzard Entertainment sont des marques de Blizzard Entertainment, Inc. Ce projet n'est ni affilié à Blizzard ni approuvé par Blizzard.")}
          </p>
        </section>
      </div>
    </div>
  );
}
