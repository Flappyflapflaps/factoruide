import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { api } from "../api";
import { useT } from "../i18n";

type Step =
  | { kind: "idle" | "checking" | "current" | "installing" | "installed" }
  | { kind: "downloading"; percent: number | null }
  | { kind: "failed"; error: string };

interface Props {
  /** A generation is running: installing waits for it to end. */
  busy: boolean;
  /** The guide editor has changes not saved. */
  unsaved: { readonly current: boolean };
}

/** Asks the update banner to check now (the About panel's button; the macOS app menu sends it too). */
export const CHECK_FOR_UPDATES = "check-for-updates";

/**
 * Offers the new version published on GitHub, with its notes: checked at startup, and again on
 * request. Published builds only: a local build has no update channel.
 */
export function UpdateBanner({ busy, unsaved }: Props) {
  const t = useT();
  const [update, setUpdate] = useState<Update | null>(null);
  const [step, setStep] = useState<Step>({ kind: "idle" });
  // Checked on request: shown, even with nothing to offer, until closed.
  const [requested, setRequested] = useState(false);
  const [restartError, setRestartError] = useState<string | null>(null);
  const banner = useRef<HTMLElement>(null);
  // A check or an install is running.
  const locked = useRef(false);
  // A check on request wins over a startup check answering late.
  const manualChecks = useRef(0);
  const resource = useRef<Update | null>(null);
  // Read when the download ends: a generation started meanwhile keeps the app open.
  const busyNow = useRef(busy);
  useEffect(() => {
    busyNow.current = busy;
  }, [busy]);

  const replaceUpdate = useCallback((next: Update | null) => {
    const previous = resource.current;
    resource.current = next;
    setUpdate(next);
    if (previous && previous !== next) void previous.close().catch(() => {});
  }, []);

  useEffect(() => {
    let cancelled = false;
    // Offline, or no release yet: nothing to offer, nothing shown.
    api
      .updatesEnabled()
      .then(async (enabled) => {
        if (!enabled || cancelled || manualChecks.current > 0) return;
        const next = await check({ timeout: 30000 });
        if (cancelled || manualChecks.current > 0) {
          if (next) await next.close();
          return;
        }
        replaceUpdate(next);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      if (resource.current) void resource.current.close().catch(() => {});
      resource.current = null;
    };
  }, [replaceUpdate]);

  const checkNow = useCallback(async () => {
    if (locked.current || !(await api.updatesEnabled().catch(() => false))) return;
    locked.current = true;
    manualChecks.current += 1;
    replaceUpdate(null);
    setRequested(true);
    setRestartError(null);
    setStep({ kind: "checking" });
    try {
      const next = await check({ timeout: 30000 });
      replaceUpdate(next);
      setStep({ kind: next ? "idle" : "current" });
    } catch (e) {
      setStep({ kind: "failed", error: String(e) });
    } finally {
      locked.current = false;
    }
  }, [replaceUpdate]);

  useEffect(() => {
    const unlisten = listen(CHECK_FOR_UPDATES, () => void checkNow());
    return () => {
      void unlisten.then((stop) => stop()).catch(() => {});
    };
  }, [checkNow]);

  useEffect(() => {
    if (requested && step.kind === "checking") banner.current?.scrollIntoView({ block: "start" });
  }, [requested, step.kind]);

  const restart = async () => {
    setRestartError(null);
    try {
      await relaunch();
    } catch (e) {
      // The update stays installed: only restarting is tried again, never a second download.
      setRestartError(String(e));
    }
  };
  const restartAsked = () => {
    if (unsaved.current && !confirm(t("Quitter l'édition sans enregistrer ? Les modifications seront perdues."))) return;
    void restart();
  };
  const install = async () => {
    if (!update || locked.current || busy) return;
    const losing = unsaved.current;
    const question = losing
      ? t("Installer la mise à jour et redémarrer Factoruide ? Les modifications non enregistrées seront perdues.")
      : t("Installer la mise à jour et redémarrer Factoruide ?");
    if (!confirm(question)) return;
    locked.current = true;
    let total = 0;
    let received = 0;
    setStep({ kind: "downloading", percent: null });
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") {
          received += event.data.chunkLength;
          setStep({ kind: "downloading", percent: total ? Math.min(100, Math.round((received / total) * 100)) : null });
        }
        if (event.event === "Finished") setStep({ kind: "installing" });
      });
      // Windows closes the app to run the installer; elsewhere it starts again here, unless a
      // generation or edits began during the download: then the restart button waits for the user.
      setStep({ kind: "installed" });
      if (!busyNow.current && (losing || !unsaved.current)) await restart();
    } catch (e) {
      setStep({ kind: "failed", error: String(e) });
    } finally {
      locked.current = false;
    }
  };
  const close = () => {
    replaceUpdate(null);
    setRequested(false);
    setRestartError(null);
    setStep({ kind: "idle" });
  };

  if (!update && !requested) return null;
  const working = step.kind === "checking" || step.kind === "downloading" || step.kind === "installing";
  return (
    <section ref={banner} className="update-banner" aria-label={t("Mises à jour de l'application")}>
      <div className="update-text" role="status" aria-live="polite">
        <strong>{update ? t("Version {version} disponible", { version: update.version }) : t("Mises à jour de l'application")}</strong>
        {update?.body && <div className="update-notes">{update.body}</div>}
        {step.kind === "checking" && <p>{t("Recherche de mises à jour…")}</p>}
        {step.kind === "current" && <p>{t("Vous utilisez la dernière version.")}</p>}
        {step.kind === "failed" && <div className="error">{t("Échec de la mise à jour : {error}", { error: step.error })}</div>}
        {busy && update && step.kind !== "installed" && <p>{t("Attendez la fin de la génération avant de mettre à jour.")}</p>}
        {restartError && <div className="error">{t("Redémarrage impossible : {error}", { error: restartError })}</div>}
      </div>
      {step.kind === "downloading" && (
        <span>{step.percent === null ? t("Téléchargement…") : t("Téléchargement… {percent} %", { percent: step.percent })}</span>
      )}
      {step.kind === "installing" && <span>{t("Installation…")}</span>}
      {step.kind === "installed" && (
        <button className="primary" disabled={busy} onClick={restartAsked}>
          {t("Redémarrer Factoruide")}
        </button>
      )}
      {update && (step.kind === "idle" || step.kind === "failed") && (
        <button className="primary" disabled={busy} onClick={install}>
          {t("Mettre à jour")}
        </button>
      )}
      {!update && step.kind === "failed" && <button onClick={() => void checkNow()}>{t("Rechercher des mises à jour")}</button>}
      {!working && <button onClick={close}>{update ? t("Plus tard") : t("Fermer")}</button>}
    </section>
  );
}
