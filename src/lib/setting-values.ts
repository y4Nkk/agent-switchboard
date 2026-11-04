import type { ConfigValue, SettingValue } from "../api/settings";

function sameConfigValue(left: ConfigValue, right: ConfigValue): boolean {
  if (left === right) return true;
  if (Array.isArray(left)) return Array.isArray(right) && left.length === right.length
    && left.every((value, index) => sameConfigValue(value, right[index]));
  if (typeof left !== "object" || typeof right !== "object" || Array.isArray(right)) return false;
  const keys = Object.keys(left);
  return keys.length === Object.keys(right).length && keys.every((key) =>
    Object.hasOwn(right, key) && sameConfigValue(left[key], right[key]));
}

export function sameSettingValue(left: SettingValue, right: SettingValue): boolean {
  return left.mode === right.mode && (left.mode === "automatic"
    || (right.mode === "explicit" && sameConfigValue(left.value, right.value)));
}
