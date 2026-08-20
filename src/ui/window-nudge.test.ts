import { describe, expect, it } from "vitest";
import {
  arrowNudgeDelta,
  shouldNudgeWindow,
  WINDOW_NUDGE_PX,
  WINDOW_NUDGE_SHIFT_PX,
} from "./window-nudge";

function key(
  partial: Partial<KeyboardEvent> & Pick<KeyboardEvent, "key">,
): KeyboardEvent {
  return {
    shiftKey: false,
    ctrlKey: false,
    altKey: false,
    metaKey: false,
    target: null,
    ...partial,
  } as KeyboardEvent;
}

describe("arrowNudgeDelta", () => {
  it("moves 4px on each arrow", () => {
    expect(arrowNudgeDelta("ArrowLeft")).toEqual({ dx: -WINDOW_NUDGE_PX, dy: 0 });
    expect(arrowNudgeDelta("ArrowRight")).toEqual({ dx: WINDOW_NUDGE_PX, dy: 0 });
    expect(arrowNudgeDelta("ArrowUp")).toEqual({ dx: 0, dy: -WINDOW_NUDGE_PX });
    expect(arrowNudgeDelta("ArrowDown")).toEqual({ dx: 0, dy: WINDOW_NUDGE_PX });
  });

  it("uses 16px with Shift", () => {
    expect(arrowNudgeDelta("ArrowLeft", true)).toEqual({
      dx: -WINDOW_NUDGE_SHIFT_PX,
      dy: 0,
    });
  });

  it("ignores other keys", () => {
    expect(arrowNudgeDelta("Escape")).toBeNull();
    expect(arrowNudgeDelta("a")).toBeNull();
  });
});

describe("shouldNudgeWindow", () => {
  it("allows body arrows", () => {
    expect(shouldNudgeWindow(key({ key: "ArrowLeft" }), { settingsOpen: false })).toBe(
      true,
    );
  });

  it("blocks when settings is open", () => {
    expect(shouldNudgeWindow(key({ key: "ArrowDown" }), { settingsOpen: true })).toBe(
      false,
    );
  });

  it("blocks ctrl/alt/meta chords", () => {
    expect(
      shouldNudgeWindow(key({ key: "ArrowLeft", ctrlKey: true }), {
        settingsOpen: false,
      }),
    ).toBe(false);
  });
});
