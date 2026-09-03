import { MatrixStarlightCanvas, type MatrixStarlightVariant } from "./MatrixStarlightCanvas";

/**
 * Activation backdrop (spiralcoder recommendation-card replica): the
 * starlight matrix and the corner glow rise together with the active flag.
 * Pure decoration — aria-hidden, pointer-transparent, self-clipped so the
 * glow never bleeds past the host surface.
 */
export function StarlightLayer({
  active,
  variant,
}: {
  active: boolean;
  variant: MatrixStarlightVariant;
}) {
  return (
    <span
      className="asb-starlight"
      data-active={active ? "true" : "false"}
      data-variant={variant}
      aria-hidden="true"
    >
      <MatrixStarlightCanvas variant={variant} />
      <span className="asb-starlight-glow" />
    </span>
  );
}
