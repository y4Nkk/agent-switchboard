import { useId, useMemo, type SVGProps } from "react";
import master from "../assets/app-icon.svg?raw";

const contents = master.slice(master.indexOf(">") + 1, master.lastIndexOf("</svg>"));

/** Inline the trusted brand master so the shared motion preference reaches it. */
export function AppLogo(props: Omit<SVGProps<SVGSVGElement>, "children" | "dangerouslySetInnerHTML" | "viewBox">) {
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const markup = useMemo(() => ({
    __html: contents.replace(/(id="|href="#|url\(#)([\w-]+)/g,
      (_, reference: string, target: string) => `${reference}asb-${id}-${target}`),
  }), [id]);
  return <svg viewBox="0 0 512 512" aria-hidden="true" focusable="false"
    {...props} dangerouslySetInnerHTML={markup} />;
}
