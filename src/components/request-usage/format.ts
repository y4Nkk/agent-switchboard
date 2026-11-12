import { formatTokenCount } from "../../lib/token-format";

export const number = (value: number | null, language: string) => value === null ? "—"
  : new Intl.NumberFormat(language, { maximumFractionDigits: 1 }).format(value);
export const duration = (ms: number | null, language: string) => ms === null ? "—"
  : ms < 1000 ? `${number(ms, language)} ms` : `${number(ms / 1000, language)} s`;
export const percent = (value: number | null, language: string) => value === null ? "—" : `${number(value, language)}%`;
export const tokens = (value: number | null) => value === null ? "—" : formatTokenCount(value);
export const speed = (value: number | null, language: string) => value === null ? "—" : `${number(value, language)} tok/s`;
export const cost = (value: string | number | null, language: string) => value === null ? "—"
  : new Intl.NumberFormat(language, { style: "currency", currency: "USD", minimumFractionDigits: 2,
    maximumFractionDigits: 6 }).format(Number(value));
