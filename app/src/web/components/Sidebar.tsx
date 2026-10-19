import { useRef, useState } from "react";
import { api } from "../../api";
import { LANGS, useLang, useT, type Lang } from "../../i18n";
import { THEMES, applyTheme, savedTheme, type Theme } from "../../theme";
import { getBackend } from "../host";
import { download } from "../mode";

/** What a backup file holds: every file the app keeps in the browser, base64. */
interface BackupFile {
  format: "factoruide-web-backup";
  version: 1;
  files: Record<string, string>;
}

const toBase64 = (buf: ArrayBuffer) => {
  const bytes = new Uint8Array(buf);
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
};
const fromBase64 = (text: string) => Uint8Array.from(atob(text), (c) => c.charCodeAt(0)).buffer;

/**
 * Web build, at the bottom of the menu: the app's language and theme, and the backup of what
 * the app keeps in this browser (guides, characters), downloaded or restored from a file.
 */
export function WebSidebarTools({ onLanguage }: { onLanguage: (lang: Lang) => void }) {
  const t = useT();
  const lang = useLang();
  const [theme, setTheme] = useState<Theme>(savedTheme);
  const [error, setError] = useState<string | null>(null);
  const picker = useRef<HTMLInputElement>(null);

  const changeLanguage = async (next: Lang) => {
    onLanguage(next);
    const s = await api.getSettings().catch(() => null);
    if (s) await api.setSettings({ ...s, language: next }).catch(() => {});
  };
  const save = async () => {
    setError(null);
    try {
      const backend = getBackend();
      await backend.flushed();
      const files = await backend.backup();
      const out: BackupFile = { format: "factoruide-web-backup", version: 1, files: Object.fromEntries(Object.entries(files).map(([k, v]) => [k, toBase64(v)])) };
      download(`factoruide-${new Date().toISOString().slice(0, 10)}.fgbackup`, JSON.stringify(out), "application/json");
    } catch (e) {
      setError(String(e));
    }
  };
  const restore = async (file: File) => {
    setError(null);
    try {
      const parsed = JSON.parse(await file.text()) as BackupFile;
      if (parsed.format !== "factoruide-web-backup") throw new Error(t("Ce fichier n'est pas une sauvegarde de Factoruide."));
      if (!confirm(t("Remplacer les guides, personnages et réglages de ce navigateur par ceux de la sauvegarde ?"))) return;
      await getBackend().restore(Object.fromEntries(Object.entries(parsed.files).map(([k, v]) => [k, fromBase64(v)])));
      location.reload();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="web-sidebar-tools">
      <select aria-label={t("Langue de l'application")} value={lang} onChange={(e) => void changeLanguage(e.target.value as Lang)}>
        {LANGS.map((l) => (
          <option key={l.key} value={l.key}>
            {l.label}
          </option>
        ))}
      </select>
      <select
        aria-label={t("Thème")}
        value={theme}
        onChange={(e) => {
          const next = e.target.value as Theme;
          setTheme(next);
          applyTheme(next);
        }}
      >
        {THEMES.map((th) => (
          <option key={th.key} value={th.key}>
            {t(th.label)}
          </option>
        ))}
      </select>
      <div className="web-sidebar-row">
        <button onClick={save} title={t("Vos guides et personnages sont gardés dans ce navigateur seulement : une sauvegarde les garde, ou les emmène dans un autre navigateur.")}>
          {t("Sauvegarder")}
        </button>
        <button onClick={() => picker.current?.click()}>{t("Restaurer…")}</button>
        <input
          ref={picker}
          type="file"
          accept=".fgbackup,application/json"
          hidden
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void restore(f);
            e.target.value = "";
          }}
        />
      </div>
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
