import { useCallback, useEffect, useState } from "react";
import { api, fmtTime, type GuideSummary, type GuideVersion, type Options } from "../api";
import { Character } from "./Character";
import { GuideViewer } from "./GuideViewer";
import { fmtGain } from "./OptimizeProgress";
import { RouteSummary } from "./RouteSummary";
import { useDate, useT } from "../i18n";
import { Icon } from "./Icons";
import { IssueForm } from "./IssueForm";

interface Props {
  version: GuideVersion;
  /** Compute the same configuration again (new data, new algorithm): a new version. */
  onRerun: (version: GuideVersion) => void;
  /** Start a new configuration from this one. */
  onEdit: (version: GuideVersion) => void;
  /** A new guide from the same configuration (under another name). */
  onClone?: (version: GuideVersion) => void;
  /** Edit the guide's steps by hand (a new version once saved). */
  onEditSteps?: (version: GuideVersion) => void;
  /** Show another version of the guide. */
  onOpenVersion?: (n: number) => void;
  /** Compare two versions of the guide (`a` the older one). */
  onCompare?: (a: number, b: number) => void;
  /** Options of the game version (race names and factions), when loaded. */
  options?: Options | null;
}

/** Time difference as "identique", "−3 min" (faster) or "+1h02" (slower). */
export function Delta({ seconds }: { seconds: number }) {
  const t = useT();
  if (Math.abs(seconds) < 60) return <span>{t("identique")}</span>;
  return <span className={seconds < 0 ? "better" : "worse"}>{seconds < 0 ? fmtGain(-seconds) : `+${fmtTime(seconds)}`}</span>;
}

/** A guide version: watched on its map (steps, character, time), its figures and versions, and what to do with it. */
export function ResultView({ version, onRerun, onEdit, onClone, onEditSteps, onOpenVersion, onCompare, options }: Props) {
  const t = useT();
  const date = useDate();
  const { meta, outcome } = version;
  const route = outcome.route;
  const [guide, setGuide] = useState<GuideSummary | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The map and steps, or the figures and versions.
  const [tab, setTab] = useState<"map" | "summary">("map");
  // A problem report open, and the steps checked for it.
  const [reporting, setReporting] = useState(false);
  const [reported, setReported] = useState<Set<number>>(new Set());
  const toggleReported = (k: number) =>
    setReported((r) => {
      const next = new Set(r);
      if (!next.delete(k)) next.add(k);
      return next;
    });
  // The steps of a card, all checked or none.
  const toggleCard = (steps: number[]) =>
    setReported((r) => {
      const next = new Set(r);
      const on = !steps.every((k) => r.has(k));
      for (const k of steps) {
        if (on) next.add(k);
        else next.delete(k);
      }
      return next;
    });

  const loadGuide = useCallback(() => {
    api.getGuide(meta.guide).then(setGuide).catch(() => setGuide(null));
  }, [meta.guide]);
  useEffect(loadGuide, [loadGuide]);

  const act = async (action: () => Promise<string | void>) => {
    setError(null);
    setMessage(null);
    try {
      const m = await action();
      if (m) setMessage(m);
      loadGuide();
    } catch (e) {
      setError(String(e));
    }
  };
  // The version the guide's characters follow (the latest unless one is chosen).
  const follow = (n: number) => act(async () => t("Guides dans l'addon : {n} — faites /reload en jeu", { n: await api.setInstalled(meta.guide, n) }));

  const versions = guide?.versions ?? [];
  const previous = versions.find((v) => v.version < meta.version);
  const followed = guide?.installed ?? versions[0]?.version;
  const installed = followed === meta.version;
  return (
    <div className="result-view">
      <header className="result-head compact">
        <div className="rh-title">
          <h2>
            {route.name} <span className="version-tag">v{meta.version}</span>
            {installed && versions.length > 1 && <span className="tag installed">{t("suivie")}</span>}
          </h2>
          <div className="muted result-meta">
            <Character race={version.request.race} klass={version.request.class} group={version.request.group} options={options} /> ·{" "}
            {t("niveau {from} → {to}", { from: route.from_level, to: route.to_level })} · {route.total_time_text} ·{" "}
            <span title={meta.data ? t("Données : {data}", { data: meta.data }) : undefined}>{t("Calculé le {date}", { date: date(meta.created) })}</span>
            {meta.imported && <> · {t("importé")}</>}
            {meta.edited && <> · {t("modifié à la main")}</>}
            {meta.character && <> · {t("depuis {name}", { name: meta.character })}</>}
          </div>
        </div>
        <div className="actions compact">
          {!installed && (
            <button className="primary" onClick={() => follow(meta.version)} title={t("Les personnages de ce guide suivront cette version en jeu.")}>
              {t("Suivre cette version")}
            </button>
          )}
          <button className="icon-only" onClick={() => onRerun(version)} title={`${t("Relancer le calcul")} : ${t("Même configuration, données et algorithme actuels : nouvelle version")}`} aria-label={t("Relancer le calcul")}>
            <Icon name="rerun" />
          </button>
          <button className="icon-only" onClick={() => onEdit(version)} title={t("Modifier la configuration")} aria-label={t("Modifier la configuration")}>
            <Icon name="config" />
          </button>
          {onEditSteps && (
            <button
              className="icon-only"
              onClick={() => onEditSteps(version)}
              title={`${t("Modifier les étapes")} : ${t("Déplacer, supprimer ou ajouter des étapes ; le guide est rejoué à chaque modification")}`}
              aria-label={t("Modifier les étapes")}
            >
              <Icon name="steps" />
            </button>
          )}
          {onClone && (
            <button className="icon-only" onClick={() => onClone(version)} title={`${t("Cloner")} : ${t("Nouveau guide à partir de la même configuration")}`} aria-label={t("Cloner")}>
              <Icon name="clone" />
            </button>
          )}
          <button
            className="icon-only"
            onClick={() => act(async () => t("Fichier à partager enregistré : {path}", { path: await api.exportVersion(meta.guide, meta.version) }))}
            title={`${t("Partager")} : ${t("Enregistre un fichier .fgguide (configuration et guide) à envoyer aux autres joueurs")}`}
            aria-label={t("Partager")}
          >
            <Icon name="share" />
          </button>
          <button
            className="icon-only"
            onClick={() => setReporting(true)}
            disabled={reporting}
            title={`${t("Signaler un problème")} : ${t("Envoyer un signalement sur ce guide, avec les étapes concernées")}`}
            aria-label={t("Signaler un problème")}
          >
            <Icon name="report" />
          </button>
        </div>
        <div className="view-tabs" role="tablist">
          <button role="tab" aria-selected={tab === "map"} className={tab === "map" ? "active" : ""} onClick={() => setTab("map")}>
            {t("Carte et étapes")}
          </button>
          <button role="tab" aria-selected={tab === "summary"} className={tab === "summary" ? "active" : ""} onClick={() => setTab("summary")}>
            {t("Résumé")}
            {versions.length > 1 && <span className="badge">{versions.length}</span>}
          </button>
        </div>
      </header>
      {reporting && (
        <IssueForm
          guide={{ guide: meta.guide, version: meta.version, steps: route.steps }}
          selected={[...reported]}
          onUnselect={toggleReported}
          onSent={() => setReported(new Set())}
          onClose={() => {
            setReporting(false);
            setReported(new Set());
          }}
        />
      )}
      {message && <div className="success">{message}</div>}
      {error && <div className="error">{error}</div>}

      {tab === "map" ? (
        <GuideViewer
          key={`${meta.guide}-${meta.version}`}
          version={version}
          onEditSteps={onEditSteps && (() => onEditSteps(version))}
          checked={reporting ? reported : undefined}
          onCheck={reporting ? toggleCard : undefined}
        />
      ) : (
        <>
          <RouteSummary
            route={route}
            notes={outcome.notes}
            compare={
              previous && (
                <>
                  <Delta seconds={meta.total_time - previous.total_time} /> {t("vs version {n}", { n: previous.version })}
                  {onCompare && (
                    <>
                      {" · "}
                      <button className="link" onClick={() => onCompare(previous.version, meta.version)}>
                        {t("Comparer")}
                      </button>
                    </>
                  )}
                </>
              )
            }
          />
          {versions.length > 1 && (
            <section className="panel">
              <h3>{t("Versions")}</h3>
              <table className="table versions">
                <tbody>
                  {versions.map((v, i) => {
                    const before = versions[i + 1];
                    return (
                      <tr key={v.version} className={v.version === meta.version ? "selected" : ""}>
                        <td>
                          {onOpenVersion && v.version !== meta.version ? (
                            <button className="link" onClick={() => onOpenVersion(v.version)}>
                              v{v.version}
                            </button>
                          ) : (
                            <strong>v{v.version}</strong>
                          )}
                          {followed === v.version && <span className="tag installed">{t("suivie")}</span>}
                          {v.imported && <span className="tag">{t("importé")}</span>}
                          {v.edited && <span className="tag">{t("modifiée")}</span>}
                        </td>
                        <td>{date(v.created)}</td>
                        <td>{v.total_time_text}</td>
                        <td>{before ? <Delta seconds={v.total_time - before.total_time} /> : ""}</td>
                        <td className="row-actions">
                          {onCompare && v.version !== meta.version && (
                            <button
                              className="link"
                              title={t("Les étapes des deux versions côte à côte, leurs différences marquées")}
                              onClick={() => onCompare(Math.min(v.version, meta.version), Math.max(v.version, meta.version))}
                            >
                              {t("Comparer")}
                            </button>
                          )}
                          {followed !== v.version && (
                            <button className="link" onClick={() => follow(v.version)}>
                              {t("Suivre")}
                            </button>
                          )}
                          {v.version !== meta.version && (
                            <button
                              className="link danger"
                              onClick={() => {
                                if (confirm(t("Supprimer la version {n} ?", { n: v.version }))) act(() => api.deleteVersion(meta.guide, v.version));
                              }}
                            >
                              {t("Supprimer")}
                            </button>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </section>
          )}
        </>
      )}
    </div>
  );
}
