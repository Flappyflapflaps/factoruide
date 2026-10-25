import { useMemo, useState } from "react";
import type { Options } from "../api";
import { useGameName, useT } from "../i18n";

interface Props {
  options: Options;
  /** Make the guide of this race and class (the import runs and says how it went). */
  onImport: (race: string, klass: string) => void;
  onClose: () => void;
  /** What the import makes (default: RestedXP's). */
  intro?: string;
}

/** The race and class a guide is made for from the RestedXP guides of the player's RXPGuides addon (or from another addon's, said in `intro`). */
export function RxpImport({ options, onImport, onClose, intro }: Props) {
  const t = useT();
  const gameName = useGameName();
  const [race, setRace] = useState(options.races[0]?.key ?? "");
  const raceInfo = options.races.find((r) => r.key === race);
  const classes = useMemo(() => options.classes.filter((c) => raceInfo?.classes.includes(c.key)), [options.classes, raceInfo]);
  const [klass, setKlass] = useState(classes[0]?.key ?? "");
  const factions = useMemo(() => [...new Set(options.races.map((r) => r.faction))], [options.races]);
  const chosen = classes.some((c) => c.key === klass) ? klass : (classes[0]?.key ?? "");
  return (
    <div className="rxp-import">
      <p>
        {intro ??
          t(
            "Fait un guide des guides RestedXP gratuits de l'addon RestedXP Guides installé dans le jeu (RestedXP, CC BY-NC-SA 4.0), rejoués avec les données de Factoruide : les étapes impossibles sont retirées.",
          )}
      </p>
      <label className="field">
        <span>{t("Race")}</span>
        <select value={race} onChange={(e) => setRace(e.target.value)}>
          {factions.map((f) => (
            <optgroup key={f} label={gameName(f)}>
              {options.races
                .filter((r) => r.faction === f)
                .map((r) => (
                  <option key={r.key} value={r.key}>
                    {gameName(r.name)}
                  </option>
                ))}
            </optgroup>
          ))}
        </select>
      </label>
      <label className="field">
        <span>{t("Classe")}</span>
        <select value={chosen} onChange={(e) => setKlass(e.target.value)}>
          {classes.map((c) => (
            <option key={c.key} value={c.key}>
              {gameName(c.name)}
            </option>
          ))}
        </select>
      </label>
      <div className="rxp-import-actions">
        <button className="primary" disabled={!race || !chosen} onClick={() => onImport(race, chosen)}>
          {t("Importer")}
        </button>
        <button onClick={onClose}>{t("Annuler")}</button>
      </div>
    </div>
  );
}
