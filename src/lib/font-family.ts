/**
 * Single owner of the interface-font → CSS conversion. A settings font name
 * is a plain family name validated by the backend; it must always be quoted
 * when placed in a font-family value so commas and digits inside a name
 * cannot split it into extra families.
 */
export function quotedFontFamily(name: string): string {
  return `"${name.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}
