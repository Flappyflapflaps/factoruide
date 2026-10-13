import type { Params } from "../api";
import type { Field } from "../paramSchema";
import { bcp47, useLang, useT } from "../i18n";
import { NumberInput } from "./NumberInput";
import { Hint } from "./Hint";

type Value = boolean | number | string;

export type SettingRowProps = {
  /** Name and explanation, in French (translated here). */
  label: string;
  help?: string;
  value: Value;
  /** The default: marked on the slider, offered to go back to. */
  initial: Value;
  onChange: (value: Value) => void;
} & (
  | { kind: "bool" }
  | { kind: "select"; options: { value: string; label: string }[] }
  /** Shown divided by `scale` (minutes for a value stored in seconds, % for a share) in `unit`. */
  | { kind: "number"; min?: number; max?: number; step?: number; unit?: string; scale?: number }
);

/**
 * One setting on a line: its name, its control (a slider when it has bounds, with a mark at the
 * default; a switch; a list), and when changed, a dot before the name and the default to go back
 * to.
 */
export function SettingRow(props: SettingRowProps) {
  const { label, help, value, initial, onChange } = props;
  const t = useT();
  const lang = useLang();
  const changed = JSON.stringify(value) !== JSON.stringify(initial);
  const scale = props.kind === "number" ? (props.scale ?? 1) : 1;
  const shown = (v: number) => Math.round((v / scale) * 1000) / 1000;
  const unit = props.kind === "number" && props.unit ? t(props.unit) : "";
  const text = (v: Value) =>
    props.kind === "bool"
      ? v
        ? "✓"
        : "✕"
      : props.kind === "select"
        ? t(props.options.find((o) => o.value === v)?.label ?? String(v)).split(/ [(:]/)[0]
        : `${new Intl.NumberFormat(bcp47(lang), { maximumFractionDigits: 3, useGrouping: false }).format(shown(v as number))}${unit && unit !== "×" ? " " : ""}${unit}`;

  let control;
  if (props.kind === "bool") {
    control = (
      <label className="switch">
        <input type="checkbox" checked={Boolean(value)} aria-label={t(label)} onChange={(e) => onChange(e.target.checked)} />
        <span />
      </label>
    );
  } else if (props.kind === "select") {
    control = (
      <select value={String(value)} aria-label={t(label)} onChange={(e) => onChange(e.target.value)}>
        {props.options.map((o) => (
          <option key={o.value} value={o.value}>
            {t(o.label)}
          </option>
        ))}
      </select>
    );
  } else {
    const { min, max, step } = props;
    const bounded = min != null && max != null && max > min;
    // Where the default sits on the slider (the thumb is 14 px wide: its centre runs from 7 px).
    const at = bounded ? Math.min(1, Math.max(0, (shown(initial as number) - min) / (max - min))) : 0;
    control = (
      <>
        {bounded ? (
          <span className="setting-range">
            <input type="range" min={min} max={max} step={step} value={shown(value as number)} aria-label={t(label)} onChange={(e) => onChange(Number(e.target.value) * scale)} />
            <i className="setting-default" style={{ left: `calc(7px + (100% - 14px) * ${at})` }} />
          </span>
        ) : (
          <span />
        )}
        <NumberInput value={shown(value as number)} min={min} max={max} step={step} aria-label={t(label)} onChange={(v) => onChange(v * scale)} />
        <em className="setting-unit">{unit}</em>
      </>
    );
  }

  return (
    <div className={`setting setting-${props.kind}${changed ? " changed" : ""}`}>
      <span className="setting-label">
        {t(label)}
        {help && <Hint text={t(help)} />}
      </span>
      {control}
      {changed ? (
        <button className="setting-reset" title={t("Valeur par défaut")} onClick={() => onChange(initial)}>
          ↺ {text(initial)}
        </button>
      ) : (
        <span className="setting-reset" />
      )}
    </div>
  );
}

interface Props {
  field: Field;
  params: Params;
  defaults: Params;
  onChange: (key: keyof Params, value: Params[keyof Params]) => void;
}

/** A planner setting (from the settings' schema) on a line. */
export function ParamField({ field, params, defaults, onChange }: Props) {
  const common = {
    label: field.label,
    help: field.help,
    value: params[field.key] as Value,
    initial: defaults[field.key] as Value,
    onChange: (v: Value) => onChange(field.key, v as Params[keyof Params]),
  };
  if (field.kind === "bool") return <SettingRow {...common} kind="bool" />;
  if (field.kind === "select") return <SettingRow {...common} kind="select" options={field.options} />;
  return <SettingRow {...common} kind="number" min={field.min} max={field.max} step={field.step} unit={field.unit} scale={field.scale} />;
}
