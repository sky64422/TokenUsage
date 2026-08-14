/**
 * True glass height (header + provider cards), ignoring window clamp.
 * Settings is an absolute overlay and must not inflate this measure.
 *
 * Ported from EconomyWarRoom: getBoundingClientRect under max-height:100% shrinks
 * with the window, so setMinSize must use unconstrained content height.
 */

/** Floor for user resize — content still reads at ~240 (was 280). */
const POLICY_MIN_W = 240;
const CHROME_MIN_H = 110;

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
    // Sum pieces. Do not use the panel rect — it stretches to the HWND.
    // Include panel border + slack: missing 1–2px makes .content overflow-y
    // paint a scrollbar (inner box = window − header − borders − padding).
    const header = panel.querySelector<HTMLElement>("#header-root");
    const list = panel.querySelector<HTMLElement>(".provider-list");
    const empty = panel.querySelector<HTMLElement>(".empty-state");
    const content = panel.querySelector<HTMLElement>(".content");
    const panelCs = getComputedStyle(panel);
    const panelChrome =
      (parseFloat(panelCs.borderTopWidth) || 0) +
      (parseFloat(panelCs.borderBottomWidth) || 0);
    let pad = 10;
    if (content) {
      const cs = getComputedStyle(content);
      pad =
        (parseFloat(cs.paddingTop) || 0) +
        (parseFloat(cs.paddingBottom) || 0);
    }
    const bodyH = Math.ceil(
      list?.scrollHeight ||
        empty?.scrollHeight ||
        list?.getBoundingClientRect().height ||
        0,
    );
    const headerH = Math.ceil(
      header?.getBoundingClientRect().height ?? header?.offsetHeight ?? 38,
    );
    const hug = headerH + bodyH + Math.ceil(pad) + Math.ceil(panelChrome) + 2;
    return hug >= 80 ? hug : headerH + bodyH + Math.ceil(pad) + 6;
  } finally {
    for (const s of saved) {
      s.el.style.maxHeight = s.maxHeight;
      s.el.style.height = s.height;
      s.el.style.overflow = s.overflow;
    }
  }
}

export { POLICY_MIN_W, CHROME_MIN_H };
