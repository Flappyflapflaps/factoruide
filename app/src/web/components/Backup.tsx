import { useState } from "react";
import { useT } from "../../i18n";
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
 * Web build: the guides, characters and settings live in this browser only. A backup file
 * keeps them (clearing the site's data loses them) or takes them to another browser.
 */
export function Backup() {
  const t = useT();
  const [error, setError] = useState<string | null>(null);
  const save = async () => {
    setError(null);
    try {
      const backend = getBackend();
      await backend.flushed();
      const files = await backend.backup();
      const out: BackupFile = { format: "factoruide-web-backup", version: 1, files: Object.fromEntries(Object.entries(files).map(([k, v]) => [k, toBase64(v)])) };
      const day = new Date().toISOString().slice(0, 10);
      download(`factoruide-${day}.fgbackup`, JSON.stringify(out), "application/json");
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
    <section className="panel web-backup">
      <h3>{t("Sauvegarde")}</h3>
      <p className="muted">{t("Vos guides, personnages et réglages sont gardés dans ce navigateur seulement : effacer les données du site les efface. Une sauvegarde les garde, ou les emmène dans un autre navigateur.")}</p>
      <div className="row">
        <button onClick={save}>{t("Télécharger une sauvegarde")}</button>
        <label className="button">
          {t("Restaurer une sauvegarde…")}
          <input
            type="file"
            accept=".fgbackup,application/json"
            hidden
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) void restore(f);
              e.target.value = "";
            }}
          />
        </label>
      </div>
      {error && <div className="error">{error}</div>}
    </section>
  );
}
