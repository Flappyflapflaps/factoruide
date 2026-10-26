import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useT } from "../i18n";

/** A re-plan asked in game ("Re-plan from here", the addon): started, installed or failed. */
interface ReplanEvent {
  character: string;
  state: "planning" | "ready" | "failed";
  guide: string | null;
  message: string | null;
}

/** The event the app sends while it computes a route again for a character (replan.rs). */
const REPLAN = "replan";

/**
 * Over every page: a character asked in game to re-plan from where it stands. While the app
 * computes the route again in the background, then once it is installed (a /reload in game loads
 * it), or why it failed. Closed by hand.
 */
export function ReplanBanner() {
  const t = useT();
  const [event, setEvent] = useState<ReplanEvent | null>(null);
  useEffect(() => {
    const unlisten = listen<ReplanEvent>(REPLAN, (e) => setEvent(e.payload));
    return () => {
      void unlisten.then((stop) => stop()).catch(() => {});
    };
  }, []);
  if (!event) return null;
  const who = event.character.split("-")[0];
  const text =
    event.state === "planning"
      ? t("{name} a demandé un nouveau plan en jeu : calcul du parcours depuis sa position…", { name: who })
      : event.state === "ready"
        ? t("Nouveau parcours de {name} prêt et installé : tapez /reload en jeu pour le suivre.", { name: who })
        : t("Le nouveau plan de {name} a échoué : {error}", { name: who, error: event.message ?? "?" });
  return (
    <div className={event.state === "failed" ? "wow-banner" : "update-banner"} role="status">
      <span className="wow-banner-text">{text}</span>
      {event.state !== "planning" && <button onClick={() => setEvent(null)}>{t("Fermer")}</button>}
    </div>
  );
}
