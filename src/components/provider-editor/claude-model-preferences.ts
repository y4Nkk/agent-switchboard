export type Fragment = Record<string, unknown>;
export type ModelPreference = "effortLevel" | "maxEffortLevel" | "autoCompactWindow";
export const modeledPreferences: ModelPreference[] = ["effortLevel", "maxEffortLevel", "autoCompactWindow"];

export function object(value: unknown): Fragment {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Fragment : {};
}

/** Update one modeled leaf without replacing a model's other native preferences. */
export function withModelPreference(fragment: Fragment, model: string, key: ModelPreference, value: string | number | undefined): Fragment {
  let models = { ...object(fragment.modelSettings) };
  const preferences = { ...object(models[model]) };
  if (value === undefined) delete preferences[key];
  else preferences[key] = value;
  if (Object.keys(preferences).length) models = { ...models, [model]: preferences };
  else delete models[model];
  const next = { ...fragment };
  if (Object.keys(models).length) next.modelSettings = models;
  else delete next.modelSettings;
  return next;
}

export function withoutModelPreferences(fragment: Fragment): Fragment {
  return Object.keys(object(fragment.modelSettings)).reduce((next, model) =>
    modeledPreferences.reduce((draft, key) => withModelPreference(draft, model, key, undefined), next), fragment);
}
