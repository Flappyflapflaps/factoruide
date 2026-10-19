import { useRef, useState } from "react";
import { api } from "../../api";
import { useT } from "../../i18n";

/**
 * Web build: a character comes from the game as a text: "Export my character" in game, copied, pasted
 * here. Pasting it again later updates the character (level, quests, position...).
 */
export function ProfilePaste({ onImported, compact }: { onImported: (name: string) => void; compact?: boolean }) {
  const t = useT();
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const picker = useRef<HTMLInputElement>(null);
  const reasons: Record<string, string> = {
    not_a_profile: t("Ce texte n'est pas un export de personnage : en jeu, tapez /fg, cliquez sur « Exporter mon personnage » et copiez tout le texte affiché."),
    corrupt_profile: t("Ce texte est incomplet ou abîmé : copiez-le de nouveau en entier (Ctrl+A puis Ctrl+C dans la fenêtre de l'addon)."),
  };
  const submit = async (content: string) => {
    if (!content.trim()) return;
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const r = await api.importProfile(content);
      setText("");
      setMessage(t("{name} (niveau {level}) est à jour.", { name: r.character.split("-")[0], level: r.level }));
      onImported(r.character);
    } catch (e) {
      const key = String(e).split(":")[0];
      setError(reasons[key] ?? String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className={compact ? "web-paste compact" : "panel web-paste"}>
      {!compact && <h3>{t("Ajouter ou mettre à jour un personnage")}</h3>}
      <ol className="web-steps">
        <li>{t("En jeu, connecté avec le personnage, tapez /fg et cliquez sur « Exporter mon personnage ».")}</li>
        <li>{t("Copiez le texte de la fenêtre qui s'ouvre (il est déjà sélectionné : Ctrl+C).")}</li>
        <li>{t("Collez-le ici (Ctrl+V), ou ouvrez un fichier texte qui le contient.")}</li>
      </ol>
      <textarea
        className="web-paste-box"
        value={text}
        placeholder="FGP1:…"
        spellCheck={false}
        onChange={(e) => setText(e.target.value)}
        onPaste={(e) => {
          // Pasted whole: kept at once.
          const pasted = e.clipboardData.getData("text");
          if (pasted.trim().startsWith("FGP1:") && !text.trim()) {
            e.preventDefault();
            void submit(pasted);
          }
        }}
      />
      <div className="row">
        <button className="primary" disabled={busy || !text.trim()} onClick={() => submit(text)}>
          {busy ? t("Lecture…") : t("Ajouter")}
        </button>
        <button onClick={() => picker.current?.click()} disabled={busy}>
          {t("Ouvrir un fichier…")}
        </button>
        <input
          ref={picker}
          type="file"
          accept=".txt,text/plain"
          hidden
          onChange={async (e) => {
            const f = e.target.files?.[0];
            e.target.value = "";
            if (f) await submit(await f.text());
          }}
        />
        {message && <span className="success">{message}</span>}
      </div>
      {error && <div className="error">{error}</div>}
    </section>
  );
}
