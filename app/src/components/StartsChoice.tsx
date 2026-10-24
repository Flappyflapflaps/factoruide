import { useEffect, useState } from "react";
import { fmtTime, type Options, type StartOption } from "../api";
import { useAreaName, useT } from "../i18n";
import { loadImage } from "./WorldMap";

interface Props {
  starts: StartOption[];
  /** How many starts were computed (those beginning in the same zone are shown once). */
  tried: number;
  /** The start chosen when the guide was computed before (a rerun): chosen again at first. */
  previous?: number;
  /** What was tried, shown on demand. */
  details: string[];
  zones: Options["zones"];
  /** The start chosen (the best score is the one shown first and chosen at first). */
  onPick: (start: StartOption) => void;
  onBack: () => void;
}

/** The map of a start's zone, as a picture for its card. */
function ZonePicture({ map }: { map: number }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    loadImage(map)
      .then((s) => live && setSrc(s))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [map]);
  return <div className="start-picture">{src && <img src={src} alt="" />}</div>;
}

/**
 * The ways the guide can begin, each lightly computed: the time of the guide it leads to (the
 * score the planner ranks by, smaller, in a tooltip), its steps and quests. The best score is
 * chosen at first.
 */
export function StartsChoice({ starts, tried, previous, details, zones, onPick, onBack }: Props) {
  const t = useT();
  const areaName = useAreaName();
  const [chosen, setChosen] = useState(() => Math.max(0, previous ? starts.findIndex((s) => s.start_zone === previous) : 0));
  const name = (zone: number) => {
    const z = zones.find((x) => x.id === zone);
    return z ? areaName(z) : `#${zone}`;
  };
  return (
    <section className="panel starts">
      <h2>{t("Par où commencer ?")}</h2>
      <p className="muted">
        {t("Plusieurs départs sont possibles : chacun est un guide calculé rapidement, avant d'être optimisé à fond. Le plus rapide est choisi d'office.")}{" "}
        {t("{tried} départs calculés, {shown} réellement différents : ceux qui commencent dans la même zone sont regroupés, ceux bien plus lents sont écartés.", { tried, shown: starts.length })}
      </p>
      <div className="starts-grid" role="radiogroup" aria-label={t("Départs possibles")}>
        {starts.map((s, i) => (
          <button
            key={s.zone}
            role="radio"
            aria-checked={chosen === i}
            className={`start-card${chosen === i ? " chosen" : ""}`}
            onClick={() => setChosen(i)}
            onDoubleClick={() => onPick(s)}
          >
            <ZonePicture map={s.ui_map} />
            <div className="start-body">
              <div className="start-zone">
                {name(s.zone)}
                {i === 0 && <span className="start-best">{t("Le plus rapide")}</span>}
                {previous ? s.start_zone === previous && <span className="start-previous">{t("Départ précédent")}</span> : null}
              </div>
              <div className="start-time">{fmtTime(s.time)}</div>
              <div
                className="start-score"
                title={t("Temps noté : le temps sur lequel le moteur classe les guides. Il compte en plus les vols, le grind et les changements de zone, c'est pourquoi il peut différer du temps de jeu estimé.")}
              >
                {t("noté {time}", { time: fmtTime(s.score) })}
              </div>
              <div className="start-counts">
                {t("{steps} étapes · {quests} quêtes", { steps: s.steps.toLocaleString(), quests: s.quests.toLocaleString() })}
              </div>
              <ol className="start-first">
                {s.first.slice(0, 3).map((line, k) => (
                  <li key={k}>{line}</li>
                ))}
              </ol>
            </div>
          </button>
        ))}
      </div>
      {details.length > 0 && (
        <details className="starts-details">
          <summary>{t("Détail du calcul")}</summary>
          <ul>
            {details.map((line, i) => (
              <li key={i}>{line}</li>
            ))}
          </ul>
        </details>
      )}
      <div className="starts-actions">
        <button onClick={onBack}>{t("Retour")}</button>
        <button className="primary big" onClick={() => onPick(starts[chosen])}>
          {t("Calculer le guide avec ce départ")}
        </button>
      </div>
    </section>
  );
}
