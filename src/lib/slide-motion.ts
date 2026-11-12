/** Single reader of the shared slide tokens: --asb-motion-fast, --asb-space-3, --asb-easing. */
export function readSlideMotion(element: HTMLElement) {
  const styles = getComputedStyle(element);
  const durationToken = styles.getPropertyValue("--asb-motion-fast").trim();
  const duration = Number.parseFloat(durationToken) * (durationToken.endsWith("ms") ? 1 : 1000);
  return {
    duration,
    distance: Number.parseFloat(styles.getPropertyValue("--asb-space-3")),
    easing: styles.getPropertyValue("--asb-easing").trim(),
  };
}
