import { useEffect, useState } from "react";
import { api, type HelpCheck } from "../api";
import { gameLocale, useGameName, useLang, useT } from "../i18n";

/** Answers already sent, by check id (kept on this computer: an answered check is put aside). */
const ANSWERED_KEY = "factoruide.needHelpAnswered";
const MAX_NOTE = 500;

type Answer = "yes" | "no";

const loadAnswered = (): Record<string, Answer> => {
  try {
    return JSON.parse(localStorage.getItem(ANSWERED_KEY) ?? "{}");
  } catch {
    return {};
  }
};

/**
 * What players can check in game where the quest data cannot settle it: does an NPC offer a
 * quest before another one is turned in? Each answer goes to the Factoruide server as a problem
 * report, and corrects the guides of every player.
 */
export function NeedHelpPage() {
  const t = useT();
  const lang = useLang();
  const [checks, setChecks] = useState<HelpCheck[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [answered, setAnswered] = useState(loadAnswered);
  const [showAnswered, setShowAnswered] = useState(false);

  // A new game version opens a new page (its `key`); the language reloads the names.
  useEffect(() => {
    api
      .needHelp(gameLocale(lang))
      .then(setChecks)
      .catch((e) => setError(String(e)));
  }, [lang]);

  const remember = (id: string, answer: Answer) => {
    const next = { ...answered, [id]: answer };
    localStorage.setItem(ANSWERED_KEY, JSON.stringify(next));
    setAnswered(next);
  };

  const shown = (checks ?? []).filter((c) => showAnswered || !answered[c.id]);
  const done = (checks ?? []).filter((c) => answered[c.id]).length;

  return (
    <div className="need-help-page">
      <div className="page-head">
        <h2>{t("Besoin d'aide")}</h2>
      </div>
      <p className="muted need-help-intro">
        {t("Les bases de quêtes ne savent pas tout. Si vous passez près de ces PNJ en jeu, dites-nous ce qu'ils vous proposent : chaque réponse corrige les guides de tous les joueurs.")}
      </p>
      {error && <div className="error">{t("Erreur : {error}", { error })}</div>}
      {checks === null && !error && <p className="muted">{t("Chargement…")}</p>}
      {checks !== null && checks.length === 0 && <p className="muted">{t("Rien à vérifier pour cette version du jeu.")}</p>}
      {checks !== null && checks.length > 0 && (
        <div className="need-help-bar">
          <span className="muted">{t("{done} sur {n} déjà répondues", { done, n: checks.length })}</span>
          {done > 0 && (
            <label>
              <input type="checkbox" checked={showAnswered} onChange={(e) => setShowAnswered(e.target.checked)} />{" "}
              {t("Afficher les questions répondues")}
            </label>
          )}
        </div>
      )}
      <div className="need-help-list">
        {shown.map((c) => (
          <CheckCard key={c.id} check={c} answer={answered[c.id]} onAnswered={(a) => remember(c.id, a)} />
        ))}
      </div>
    </div>
  );
}

function CheckCard({ check, answer, onAnswered }: { check: HelpCheck; answer?: Answer; onAnswered: (a: Answer) => void }) {
  const t = useT();
  const lang = useLang();
  const gameName = useGameName();
  const [note, setNote] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const giver = check.giver?.name ?? t("son donneur");
  const before = check.before.map((b) => b.name).join(t(" ou "));
  const side = check.side === "both" ? t("les deux factions") : gameName(check.side);

  const send = async (a: Answer) => {
    setSending(true);
    setError(null);
    const lines = [`[need-help ${check.id}] ${a}`, `${check.quest.name} (${check.quest.id}) before ${check.before.map((b) => b.id).join(", ")}`];
    if (note.trim()) lines.push(note.trim());
    try {
      await api.sendIssue({ kind: "other", message: lines.join("\n"), language: lang });
      onAnswered(a);
    } catch (e) {
      const reason = String(e);
      setError(reason === "rate_limited" ? t("Trop d'envois récents : réessayez plus tard.") : t("Le serveur a refusé l'envoi : {error}", { error: reason }));
    } finally {
      setSending(false);
    }
  };

  return (
    <section className="panel need-help-card">
      <h3>
        {check.quest.name} <span className="muted">· {side} · {t("niveau {n}", { n: check.min_level })}</span>
      </h3>
      <p>{t("{giver} vous propose-t-il « {quest} » avant que vous ayez rendu {before} ?", { giver, quest: check.quest.name, before })}</p>
      {check.place && (
        <p className="muted">
          {t("{giver} : {zone}, {x}, {y}", { giver, zone: check.place.zone, x: check.place.x.toFixed(1), y: check.place.y.toFixed(1) })}
        </p>
      )}
      {answer ? (
        <div className="success">{answer === "yes" ? t("Réponse envoyée : oui. Merci !") : t("Réponse envoyée : non. Merci !")}</div>
      ) : (
        <>
          <textarea
            rows={2}
            maxLength={MAX_NOTE}
            placeholder={t("Précision (facultatif) : votre niveau, ce que le PNJ proposait…")}
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
          <div className="need-help-actions">
            <button disabled={sending} onClick={() => send("yes")}>
              {t("Oui, il la propose")}
            </button>
            <button disabled={sending} onClick={() => send("no")}>
              {t("Non, pas encore")}
            </button>
          </div>
          {error && <div className="error">{error}</div>}
        </>
      )}
    </section>
  );
}
