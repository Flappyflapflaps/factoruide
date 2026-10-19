import { useCallback, useEffect, useState } from "react";
import { api, type AutoSendStatus, type ContributionPreview, type Settings } from "../api";
import { bcp47, useDate, useLang, useT } from "../i18n";
import { Hint } from "./Hint";
import { IS_WEB } from "../web/mode";

/**
 * Send this computer's quest cache and addon recordings to consolidate the quest database (a
 * panel of the Contribute page; `key` it by the game version to read its folder again). Only
 * what the server has not accepted yet goes; automatic sends are off unless turned on.
 */
export function ContributePanel() {
  const t = useT();
  const lang = useLang();
  const date = useDate();
  const kilobytes = (bytes: number) =>
    new Intl.NumberFormat(bcp47(lang), { style: "unit", unit: "kilobyte", unitDisplay: "short", maximumFractionDigits: 0 }).format(Math.max(1, bytes / 1024));
  const [preview, setPreview] = useState<ContributionPreview | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [sending, setSending] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api.contributionPreview().then(setPreview).catch((e) => setError(String(e)));
  }, []);
  useEffect(load, [load]);
  useEffect(() => {
    api.getSettings().then(setSettings).catch((e) => setError(String(e)));
  }, []);

  const send = async () => {
    setSending(true);
    setError(null);
    setMessage(null);
    try {
      const report = await api.sendContribution();
      const done =
        report.documents > 1
          ? t("Merci ! {n} envois acceptés.", { n: report.documents })
          : report.status === "already_received"
            ? t("Ces données ont déjà été envoyées.")
            : t("Merci ! Vos données ont été envoyées.");
      setMessage(report.more ? `${done} ${t("Il reste des données : la suite partira au prochain envoi.")}` : done);
    } catch (e) {
      const reason = String(e);
      if (reason === "nothing_to_send") setMessage(t("Rien à envoyer pour l'instant : jouez avec l'addon puis revenez."));
      else setError(reason === "rate_limited" ? t("Trop d'envois récents : réessayez plus tard.") : t("Le serveur a refusé l'envoi : {error}", { error: reason }));
    } finally {
      setSending(false);
      load();
    }
  };

  const setAuto = (on: boolean) => {
    if (!settings) return;
    setSettings({ ...settings, auto_contribute: on });
    api.setAutoContribute(on).catch((e) => setError(String(e)));
  };

  const autoStatus = (a: AutoSendStatus) =>
    ({
      accepted: t("envoyé"),
      already_received: t("déjà reçu"),
      rate_limited: t("trop d'envois, nouvel essai plus tard"),
      error: t("échec : {error}", { error: a.message ?? "" }),
    })[a.status] ?? a.status;

  const empty = preview && preview.quests === 0 && preview.entities === 0 && preview.events === 0;
  return (
    <section className="panel contribute">
      <h3>
        {t("Le serveur Factoruide")}
        <Hint text={t("Chaque envoi est comparé aux données déjà sûres : seules les informations confirmées par plusieurs joueurs sont intégrées.")} />
      </h3>
      <p className="muted">
        {IS_WEB
          ? t("Ce que l'addon a enregistré (quêtes prises et rendues, PNJ et positions), venu avec les exports de vos personnages, complète la base de quêtes. Plus il y a de joueurs, plus les guides sont justes.")
          : t(
              "Les quêtes que votre jeu a vues (son cache de quêtes) et ce que l'addon a enregistré (quêtes prises et rendues, PNJ et positions) complètent la base de quêtes. Plus il y a de joueurs, plus les guides sont justes.",
            )}
      </p>
      <ul className="contribute-privacy">
        <li>{t("Anonyme : ni nom de personnage, ni compte, ni heure de jeu (seulement le jour). Uniquement des données du jeu : quêtes, PNJ, objets, positions.")}</li>
        <li>{t("Un identifiant aléatoire de cette installation accompagne l'envoi, pour compter les joueurs différents.")}</li>
        <li>{t("Le serveur ne garde pas votre adresse IP.")}</li>
      </ul>
      {preview && (
        <>
          <p className="muted">{t("Ce qui partirait maintenant : seulement ce qui est nouveau depuis le dernier envoi.")}</p>
          <div className="contribute-figures">
            {/* The browser cannot read the game's caches. */}
            {!IS_WEB && <div className="tile">
              <span>{t("Quêtes du cache")}</span>
              <strong>{preview.quests}</strong>
              <small>
                {t("+ {n} PNJ et objets", { n: preview.entities })} · {preview.locales.join(", ") || "—"}
              </small>
            </div>}
            <div className="tile">
              <span>{t("Enregistrements de l'addon")}</span>
              <strong>{preview.events}</strong>
              <small>{t("{n} personnages", { n: preview.characters })}</small>
            </div>
            <div className="tile">
              <span>{t("Taille")}</span>
              <strong>{kilobytes(preview.size)}</strong>
              {!IS_WEB && <small title={preview.client_dir}>{preview.client_dir}</small>}
            </div>
          </div>
        </>
      )}
      {preview?.more && <div className="notice">{t("Plus de nouveautés qu'un envoi ne peut en contenir : ces chiffres sont ceux du premier envoi, la suite partira aux suivants.")}</div>}
      {preview && !preview.found && (
        <div className={IS_WEB ? "notice" : "error"}>
          {IS_WEB ? t("Aucun personnage pour l'instant : les enregistrements de l'addon viennent avec les exports de personnage collés dans Nouveau guide.") : t("Dossier du jeu introuvable : vérifiez le dossier World of Warcraft dans les Réglages.")}
        </div>
      )}
      {preview && !preview.url && <div className="notice">{t("L'envoi n'est pas disponible : aucun serveur de contribution n'est configuré.")}</div>}
      {preview?.retry_after && <div className="notice">{t("Le serveur demande d'attendre : prochain envoi possible le {date}.", { date: date(preview.retry_after) })}</div>}
      <div className="actions">
        <button className="primary" disabled={!preview?.url || !!empty || !!preview?.retry_after || sending} onClick={send}>
          {sending ? t("Envoi…") : t("Envoyer mes données")}
        </button>
        {preview?.last_sent && <span className="muted">{t("Dernier envoi : {date}", { date: date(preview.last_sent) })}</span>}
      </div>
      {empty && !message && <p className="muted">{t("Rien à envoyer pour l'instant : jouez avec l'addon puis revenez.")}</p>}
      {settings && (
        <label className="field-bool contribute-auto">
          <input type="checkbox" checked={Boolean(settings.auto_contribute)} onChange={(e) => setAuto(e.target.checked)} />
          <span>
            {t("Envoyer automatiquement")}
            <Hint text={t("Une fois par jour au plus (toutes les heures tant qu'un envoi n'a pas tout pris), pour chaque version du jeu installée, seulement s'il y a du nouveau, sans fenêtre ni message.")} />
          </span>
        </label>
      )}
      {preview?.automatic.map((a) => (
        <p key={a.edition} className="muted contribute-auto-status">
          {t("Dernier envoi automatique ({edition}) : {date}, {status}", { edition: a.name, date: date(a.time), status: autoStatus(a) })}
        </p>
      ))}
      {message && <div className="success">{message}</div>}
      {error && <div className="error">{error}</div>}
    </section>
  );
}
