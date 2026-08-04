/**
 * True glass height (header + provider cards), ignoring window clamp.
 * Settings is an absolute overlay and must not inflate this measure.
 *
 * Ported from EconomyWarRoom: getBoundingClientRect under max-height:100% shrinks
 * with the window, so setMinSize must use unconstrained content height.
 */

/** Floor for user resize — content still reads at ~240 (was 280). */
const POLICY_MIN_W = 240;
const CHROME_MIN_H = 140;

export function measureContentHugHeight(panel: HTMLElement): number {
  const liftSelectors = [
    panel,
    panel.querySelector<HTMLElement>(".content"),
    panel.querySelector<HTMLElement>("#content-root"),
    panel.querySelector<HTMLElement>("#providers-root"),
    panel.querySelector<HTMLElement>(".provider-list"),
  ].filter((el): el is HTMLElement => Boolean(el));

  const saved = liftSelectors.map((el) => ({
    el,
    maxHeight: el.style.maxHeight,
    height: el.style.height,
    overflow: el.style.overflow,
  }));

  try {
    for (const { el } of saved) {
      el.style.maxHeight = "none";
      el.style.height = "max-content";
      el.style.overflow = "visible";
    }
    void panel.offsetHeight;
    // ceil + 1px slack: avoids subpixel overflow / scrollbar at min size
    const hug = Math.ceil(panel.getBoundingClientRect().height) + 1;
    if (hug >= 80) return hug;

    const header = panel.querySelector<HTMLElement>("#header-root");
    const list = panel.querySelector<HTMLElement>(".provider-list");
    const empty = panel.querySelector<HTMLElement>(".empty-state");
    const content = panel.querySelector<HTMLElement>(".content");
    let pad = 20;
    if (content) {
      const cs = getComputedStyle(content);
      pad =
        (parseFloat(cs.paddingTop) || 0) +
        (parseFloat(cs.paddingBottom) || 0);
    }
    return Math.ceil(
      (header?.offsetHeight ?? 42) +
        (list?.scrollHeight ?? empty?.scrollHeight ?? 0) +
        pad +
        4,
    );
  } finally {
    for (const s of saved) {
      s.el.style.maxHeight = s.maxHeight;
      s.el.style.height = s.height;
      s.el.style.overflow = s.overflow;
    }
  }
}

export { POLICY_MIN_W, CHROME_MIN_H };
