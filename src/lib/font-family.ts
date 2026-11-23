export const WENQUANYI_ZEN_HEI = "WenQuanYi Zen Hei";
export const WENQUANYI_ZEN_HEI_ALIASES: readonly string[] = ["文泉驿正黑", "文泉驛正黑"];

export function interfaceFontFamily(name: string): string {
  if (name.toLowerCase() === WENQUANYI_ZEN_HEI.toLowerCase() || WENQUANYI_ZEN_HEI_ALIASES.includes(name)) {
    return WENQUANYI_ZEN_HEI;
  }
  return name;
}

/**
 * Single owner of the interface-font → CSS conversion. A settings font name
 * is a plain family name validated by the backend; it must always be quoted
 * when placed in a font-family value so commas and digits inside a name
 * cannot split it into extra families.
 */
export function quotedFontFamily(name: string): string {
  return `"${name.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}
