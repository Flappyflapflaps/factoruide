import { useState } from "react";
import { bcp47, useLang } from "../i18n";

interface Props {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  "aria-label"?: string;
}

/**
 * A number written with the decimal separator of the app language (a number input follows the
 * system's instead, "0,35" in an English app on a French Windows). Arrow keys step it.
 */
export function NumberInput({ value, onChange, min, max, step = 1, ...rest }: Props) {
  const lang = useLang();
  // Text being typed ("0," on the way to "0,5"), null when not editing.
  const [draft, setDraft] = useState<string | null>(null);
  const format = (v: number) => new Intl.NumberFormat(bcp47(lang), { maximumFractionDigits: 3, useGrouping: false }).format(v);
  const parse = (text: string) => Number(text.trim().replace(",", "."));
  const clamp = (v: number) => Math.min(max ?? Infinity, Math.max(min ?? -Infinity, v));
  return (
    <input
      type="text"
      inputMode="decimal"
      {...rest}
      value={draft ?? format(value)}
      onChange={(e) => {
        setDraft(e.target.value);
        const v = parse(e.target.value);
        if (e.target.value.trim() !== "" && Number.isFinite(v)) onChange(v);
      }}
      onBlur={() => {
        setDraft(null);
        if (clamp(value) !== value) onChange(clamp(value));
      }}
      onKeyDown={(e) => {
        if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
        e.preventDefault();
        // Rounded to the step's decimals (0.1 + 0.2).
        const next = clamp(Math.round((value + (e.key === "ArrowUp" ? step : -step)) * 1000) / 1000);
        setDraft(null);
        onChange(next);
      }}
    />
  );
}
