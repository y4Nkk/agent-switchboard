import type { AppKind } from "../api/client";
import claudeLogo from "../assets/logos/claude.png";
import openaiLogo from "../assets/logos/openai.svg";

/* Brand marks identify the integrated client per segment; sourced from the
   vendors' official channels (OpenAI logo vector, Claude App Store artwork).
   Trademarks belong to their respective owners. */
const CLIENT_LOGOS: Record<AppKind, string> = {
  codex: openaiLogo,
  claude: claudeLogo,
};

/** Vendor brand mark for one integrated client; decorative beside its name. */
export function ClientLogo({ app, className }: { app: AppKind; className?: string }) {
  return <img className={className} src={CLIENT_LOGOS[app]} alt="" />;
}
