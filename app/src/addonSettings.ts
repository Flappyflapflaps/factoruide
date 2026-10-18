import schema from "../../addon/settings.json";
import type { Settings } from "./api";

/** The addon's settings and the setup questions: addon/settings.json, shared with the addon. */

export type Value = boolean | number | string;

/** `only`: shown for that edition only. `requires`: a setting it depends on, greyed out while
 *  that one is off (false or "off"). Texts in French (translated where shown). */
export type AddonOption = { key: string; label: string; help?: string; only?: Settings["edition"]; requires?: string } & (
  | { kind: "bool"; default: boolean }
  | { kind: "number"; default: number; min: number; max: number; step: number; percent?: boolean }
  | { kind: "select"; default: string; options: { value: string; label: string }[] }
);

export type AddonGroup = { title: string; sections: { title?: string; options: AddonOption[] }[] };

/** One answer of a setup question: the settings it sets. `default`: the answer when nothing was chosen. */
export type SetupChoice = { label: string; help?: string; default?: boolean; values: Record<string, Value> };
export type SetupStep = { title: string; questions: { label: string; choices: SetupChoice[] }[] };

export const ADDON_GROUPS = schema.groups as AddonGroup[];
export const SETUP_STEPS = schema.wizard as SetupStep[];
export const ADDON_OPTIONS: AddonOption[] = ADDON_GROUPS.flatMap((g) => g.sections.flatMap((s) => s.options));

const byKey = new Map(ADDON_OPTIONS.map((o) => [o.key, o]));
export const addonOption = (key: string) => byKey.get(key);

/** A setting's value (its default when unset). */
export const addonValue = (values: Settings["addon"], key: string): Value => values[key] ?? byKey.get(key)?.default ?? false;

/** Whether the setting `o` depends on is on (or it depends on none). */
export const requirementMet = (values: Settings["addon"], o: AddonOption) => {
  if (!o.requires) return true;
  const v = addonValue(values, o.requires);
  return v !== false && v !== "off";
};

/** Every setting with its value, as saved. */
export const allAddonValues = (values: Settings["addon"]) => Object.fromEntries(ADDON_OPTIONS.map((o) => [o.key, addonValue(values, o.key)]));

/** The answer the settings give to a question: the choice they match, else none (set by hand). */
export const currentChoice = (values: Settings["addon"], choices: SetupChoice[]) =>
  choices.findIndex((c) => Object.entries(c.values).every(([k, v]) => addonValue(values, k) === v));
