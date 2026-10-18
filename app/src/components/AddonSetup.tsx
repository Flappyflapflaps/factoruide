import { useState } from "react";
import type { Settings } from "../api";
import { addonOption, addonValue, currentChoice, SETUP_STEPS, type Value } from "../addonSettings";
import { LANGS, useT } from "../i18n";

interface Props {
  settings: Settings;
  /** The guides' language when none was chosen (the app's). */
  defaultLocale: string;
  /** Asked for the first time: unanswered questions start on their default answer, not on the settings as they are. */
  first: boolean;
  /** The settings with the answers, to save and install. */
  onDone: (settings: Settings) => void;
  onCancel: () => void;
}

/**
 * The addon's setup: a few questions, each answer setting several options (addon/settings.json),
 * then what changes. Asked again on demand, it starts on the answers the settings give, and
 * "keep my settings" where they match none.
 */
export function AddonSetup({ settings, defaultLocale, first, onDone, onCancel }: Props) {
  const t = useT();
  const forever = settings.edition === "forever";
  const [locale, setLocale] = useState(settings.guide_locale ?? defaultLocale);
  const [guideAddon, setGuideAddon] = useState(forever ? (settings.guide_addon ?? "factoruide") : "factoruide");
  // Per step, per question: the choice taken, -1 to keep the settings as they are.
  const initial = SETUP_STEPS.map((s) =>
    s.questions.map((q) => {
      const found = currentChoice(settings.addon, q.choices);
      return found >= 0 || !first ? found : q.choices.findIndex((c) => c.default);
    }),
  );
  const [answers, setAnswers] = useState(initial);
  const [step, setStep] = useState(0);

  const polestar = guideAddon === "polestar";
  // Language first, the questions (none with Polestar, which has its own options), what changes last.
  const titles = ["Langue", ...(polestar ? [] : SETUP_STEPS.map((s) => s.title)), "Résumé"];
  const last = titles.length - 1;

  const addon = { ...settings.addon };
  if (!polestar)
    SETUP_STEPS.forEach((s, i) =>
      s.questions.forEach((q, j) => {
        const choice = q.choices[answers[i][j]];
        if (choice) Object.assign(addon, choice.values);
      }),
    );
  const next: Settings = { ...settings, addon, guide_locale: locale, guide_addon: guideAddon, addon_setup_done: true };

  const shown = (key: string, v: Value) => {
    const o = addonOption(key);
    if (o?.kind === "select") return t(o.options.find((c) => c.value === v)?.label ?? String(v));
    if (typeof v === "boolean") return v ? "✓" : "✕";
    return String(v);
  };
  const changes = Object.keys(addon)
    .filter((k) => addonValue(addon, k) !== addonValue(settings.addon, k))
    .map((k) => ({ label: t(addonOption(k)?.label ?? k), from: shown(k, addonValue(settings.addon, k)), to: shown(k, addonValue(addon, k)) }));
  const langLabel = (l: string) => LANGS.find((x) => x.locale === l)?.label ?? l;
  if (locale !== (settings.guide_locale ?? defaultLocale)) changes.unshift({ label: t("Langue de l'addon"), from: langLabel(settings.guide_locale ?? defaultLocale), to: langLabel(locale) });
  if (forever && guideAddon !== (settings.guide_addon ?? "factoruide"))
    changes.unshift({ label: t("Guide en jeu"), from: settings.guide_addon === "polestar" ? "Polestar" : t("Addon Factoruide"), to: polestar ? "Polestar" : t("Addon Factoruide") });

  const choiceButton = (key: string, active: boolean, label: string, help: string | undefined, onClick: () => void) => (
    <button key={key} role="radio" aria-checked={active} className={`setup-choice${active ? " active" : ""}`} onClick={onClick}>
      <strong>{label}</strong>
      {help && <small>{help}</small>}
    </button>
  );

  const questions = step > 0 && step < last ? SETUP_STEPS[step - 1] : null;
  return (
    <div className="wizard addon-setup">
      <ol className="wizard-steps">
        {titles.map((title, i) => (
          <li key={title} className={i === step ? "active" : i < step ? "done" : ""} aria-current={i === step ? "step" : undefined}>
            <span className="step-num" aria-hidden>
              {i < step ? "✓" : i + 1}
            </span>
            <span>{t(title)}</span>
          </li>
        ))}
      </ol>
      <section className="panel">
        {step === 0 && (
          <>
            <div className="setup-question">
              <h3>{t("Dans quelle langue ?")}</h3>
              <p className="muted">{t("Textes de l'addon, phrases du guide et noms des quêtes, PNJ, objets et zones ; anglais quand une traduction manque.")}</p>
              <select value={locale} aria-label={t("Langue de l'addon")} onChange={(e) => setLocale(e.target.value)}>
                {LANGS.map((l) => (
                  <option key={l.locale} value={l.locale}>
                    {l.label}
                  </option>
                ))}
              </select>
            </div>
            {forever && (
              <div className="setup-question">
                <h3>{t("Quel addon vous guide en jeu ?")}</h3>
                <div className="setup-choices" role="radiogroup" aria-label={t("Guide en jeu")}>
                  {choiceButton("factoruide", !polestar, t("Addon Factoruide"), t("Le guide, la boussole et l'automatisation de Factoruide."), () => setGuideAddon("factoruide"))}
                  {choiceButton("polestar", polestar, "Polestar", t("Vos guides dans Polestar, réglé dans ses propres options ; Factoruide ne fait plus qu'enregistrer votre personnage."), () => setGuideAddon("polestar"))}
                </div>
              </div>
            )}
          </>
        )}
        {questions?.questions.map((q, j) => {
          const i = step - 1;
          const pick = (c: number) => setAnswers(answers.map((row, a) => (a === i ? row.map((x, b) => (b === j ? c : x)) : row)));
          return (
            <div className="setup-question" key={q.label}>
              <h3>{t(q.label)}</h3>
              <div className="setup-choices" role="radiogroup" aria-label={t(q.label)}>
                {q.choices.map((c, k) => choiceButton(c.label, answers[i][j] === k, t(c.label), c.help && t(c.help), () => pick(k)))}
                {initial[i][j] < 0 && choiceButton("keep", answers[i][j] < 0, t("Garder mes réglages"), t("Ceux choisis à la main sur la page de l'addon."), () => pick(-1))}
              </div>
            </div>
          );
        })}
        {step === last && (
          <div className="setup-question">
            <h3>{t("Ce qui change")}</h3>
            {changes.length ? (
              <table className="setup-changes">
                <tbody>
                  {changes.map((c) => (
                    <tr key={c.label}>
                      <td>{c.label}</td>
                      <td className="muted">{c.from}</td>
                      <td aria-hidden>→</td>
                      <td>{c.to}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <p className="muted">{t("Rien : vos réglages répondent déjà à ces choix.")}</p>
            )}
            <p className="muted">{t("Tout reste modifiable en détail sur la page de l'addon, et l'assistant se relance depuis son bouton.")}</p>
          </div>
        )}
      </section>
      <div className="setup-actions">
        <button className="link" onClick={onCancel}>
          {first ? t("Passer") : t("Annuler")}
        </button>
        <span />
        {step > 0 && <button onClick={() => setStep(step - 1)}>{t("Retour")}</button>}
        {step < last ? (
          <button className="primary" onClick={() => setStep(step + 1)}>
            {t("Suivant")}
          </button>
        ) : (
          <button className="primary" onClick={() => onDone(next)}>
            {t("Enregistrer et installer dans le jeu")}
          </button>
        )}
      </div>
    </div>
  );
}
