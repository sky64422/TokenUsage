import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  meterFillPct,
  opacityToPct,
  OPACITY_MAX_PCT,
  OPACITY_MIN_PCT,
  OPACITY_STEP_PCT,
  pctToOpacity,
  snapOpacityPct,
} from "./opacity";

export interface UpdateInfo {
  current_version: string;
  version: string;
}

export interface DownloadProgress {
  version: string;
  chunk_len: number;
  content_length: number | null;
  received: number;
}

type UpdatePhase = "idle" | "downloading" | "ready";

export function renderHeader(
  root: HTMLElement,
  opts: {
    onSettings: () => void;
    onHide: () => void;
    opacity: number;
    onOpacityChange: (o: number) => void;
  },
): void {
  const initialPct = opacityToPct(opts.opacity);
  root.innerHTML = `
    <div class="header" data-tauri-drag-region>
      <div class="title">Usage</div>
      <div class="header-actions">
        <div class="opacity-slider" id="opacity-slider" role="slider"
          aria-label="Opacity" aria-valuemin="${OPACITY_MIN_PCT}"
          aria-valuemax="${OPACITY_MAX_PCT}" aria-valuenow="${initialPct}"
          aria-valuetext="${initialPct}%" tabindex="0"
          title="Opacity ${initialPct}%"
          style="--opacity-fill: ${meterFillPct(initialPct)}%">
          <span class="opacity-slider-rail" aria-hidden="true">
            <span class="opacity-slider-track">
              <span class="opacity-slider-fill"></span>
            </span>
            <span class="opacity-slider-thumb"></span>
          </span>
        </div>
        <button type="button" class="icon-btn" id="btn-update" title="Check for updates" aria-label="Check for updates">↻</button>
        <button type="button" class="icon-btn" id="btn-settings" title="Settings" aria-label="Settings">⚙</button>
        <button type="button" class="icon-btn" id="btn-hide" title="Hide" aria-label="Hide">–</button>
      </div>
    </div>
  `;

  bindOpacitySlider(root, initialPct, opts.onOpacityChange);

  root.querySelector("#btn-settings")?.addEventListener("click", (e) => {
    e.stopPropagation();
    opts.onSettings();
  });
  root.querySelector("#btn-hide")?.addEventListener("click", (e) => {
    e.stopPropagation();
    opts.onHide();
  });

  const updateBtn = root.querySelector("#btn-update") as HTMLButtonElement;
  let phase: UpdatePhase = "idle";
  let pendingVersion: string | null = null;

  const setPhase = (next: UpdatePhase, version?: string) => {
    phase = next;
    if (version) pendingVersion = version;
    updateBtn.classList.toggle("update-available", next !== "idle");
    updateBtn.classList.toggle("update-ready", next === "ready");
    updateBtn.classList.toggle("update-downloading", next === "downloading");

    if (next === "ready" && pendingVersion) {
      const title = `Update ${pendingVersion} ready — click to restart`;
      updateBtn.setAttribute("title", title);
      updateBtn.setAttribute("aria-label", title);
      updateBtn.dataset.updateVersion = pendingVersion;
    } else if (next === "downloading" && pendingVersion) {
      const title = `Downloading ${pendingVersion}…`;
      updateBtn.setAttribute("title", title);
      updateBtn.setAttribute("aria-label", title);
      updateBtn.dataset.updateVersion = pendingVersion;
    } else {
      updateBtn.setAttribute("title", "Check for updates");
      updateBtn.setAttribute("aria-label", "Check for updates");
      delete updateBtn.dataset.updateVersion;
      pendingVersion = null;
    }
  };

  updateBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    void runUpdateAction(updateBtn, () => phase, setPhase);
  });

  void listen<UpdateInfo>("update-available", (ev) => {
    const info = ev.payload;
    if (!info?.version) return;
    setPhase("downloading", info.version);
  });

  void listen<DownloadProgress>("update-download-progress", (ev) => {
    const p = ev.payload;
    if (!p?.version || phase === "ready") return;
    pendingVersion = p.version;
    updateBtn.classList.add("update-available", "update-downloading");
    if (p.content_length && p.content_length > 0) {
      const pct = Math.min(99, Math.round((p.received / p.content_length) * 100));
      updateBtn.setAttribute("title", `Downloading ${p.version}… ${pct}%`);
    } else {
      updateBtn.setAttribute("title", `Downloading ${p.version}…`);
    }
  });

  void listen<UpdateInfo>("update-ready", (ev) => {
    const info = ev.payload;
    if (!info?.version) return;
    setPhase("ready", info.version);
  });

  void listen("update-not-available", () => {
    if (phase !== "idle") setPhase("idle");
  });

  void listen<string>("update-failed", (ev) => {
    const msg = typeof ev.payload === "string" ? ev.payload : "Update failed";
    updateBtn.setAttribute("title", msg.slice(0, 120));
    updateBtn.classList.remove("update-downloading");
    window.setTimeout(() => {
      if (!updateBtn.isConnected) return;
      if (phase === "ready" && pendingVersion) {
        setPhase("ready", pendingVersion);
      } else {
        setPhase("idle");
      }
    }, 4000);
  });
}

export function setSettingsButtonActive(root: HTMLElement, active: boolean): void {
  root.querySelector("#btn-settings")?.classList.toggle("active", active);
}

function bindOpacitySlider(
  root: HTMLElement,
  initialPct: number,
  onChange: (o: number) => void,
): void {
  const slider = root.querySelector("#opacity-slider") as HTMLElement | null;
  if (!slider) return;

  let pct = snapOpacityPct(initialPct);
  let dragging = false;

  const paint = (next: number) => {
    const snapped = snapOpacityPct(next);
    slider.style.setProperty("--opacity-fill", `${meterFillPct(snapped)}%`);
    if (snapped === pct) return;
    pct = snapped;
    slider.setAttribute("aria-valuenow", String(snapped));
    slider.setAttribute("aria-valuetext", `${snapped}%`);
    slider.title = `Opacity ${snapped}%`;
    onChange(pctToOpacity(snapped));
  };

  const pctFromClientX = (clientX: number): number => {
    const rail = slider.querySelector(".opacity-slider-rail") as HTMLElement | null;
    const rect = (rail ?? slider).getBoundingClientRect();
    if (rect.width <= 0) return pct;
    const t = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    return OPACITY_MIN_PCT + t * (OPACITY_MAX_PCT - OPACITY_MIN_PCT);
  };

  if (Math.abs(initialPct / 100 - pctToOpacity(pct)) > 0.001) {
    onChange(pctToOpacity(pct));
  }

  slider.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    dragging = true;
    slider.classList.add("is-dragging");
    slider.setPointerCapture(e.pointerId);
    paint(pctFromClientX(e.clientX));
  });

  slider.addEventListener("pointermove", (e) => {
    if (!dragging) return;
    paint(pctFromClientX(e.clientX));
  });

  const endDrag = (e: PointerEvent) => {
    if (!dragging) return;
    dragging = false;
    slider.classList.remove("is-dragging");
    if (slider.hasPointerCapture(e.pointerId)) {
      slider.releasePointerCapture(e.pointerId);
    }
  };
  slider.addEventListener("pointerup", endDrag);
  slider.addEventListener("pointercancel", endDrag);

  slider.addEventListener("keydown", (e) => {
    if (e.key === "ArrowLeft" || e.key === "ArrowDown") {
      e.preventDefault();
      paint(pct - OPACITY_STEP_PCT);
    } else if (e.key === "ArrowRight" || e.key === "ArrowUp") {
      e.preventDefault();
      paint(pct + OPACITY_STEP_PCT);
    } else if (e.key === "Home") {
      e.preventDefault();
      paint(OPACITY_MIN_PCT);
    } else if (e.key === "End") {
      e.preventDefault();
      paint(OPACITY_MAX_PCT);
    }
  });

  slider.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const axis = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
      if (axis === 0) return;
      paint(pct - Math.sign(axis) * OPACITY_STEP_PCT);
    },
    { passive: false },
  );
}

async function runUpdateAction(
  btn: HTMLButtonElement,
  getPhase: () => UpdatePhase,
  setPhase: (p: UpdatePhase, version?: string) => void,
): Promise<void> {
  const phaseAtClick = getPhase();
  const version = btn.dataset.updateVersion;

  btn.disabled = true;
  btn.classList.add("busy");

  if (phaseAtClick === "downloading") {
    btn.setAttribute("title", "Still downloading…");
    window.setTimeout(() => {
      if (!btn.isConnected) return;
      btn.disabled = false;
      btn.classList.remove("busy");
      if (version) btn.setAttribute("title", `Downloading ${version}…`);
    }, 1500);
    return;
  }

  if (phaseAtClick === "ready") {
    btn.setAttribute("title", version ? `Restarting to install ${version}…` : "Restarting…");
  } else {
    btn.setAttribute("title", "Checking...");
  }

  try {
    const hasUpdate = await invoke<boolean>("check_for_updates");
    if (hasUpdate) {
      btn.setAttribute("title", "Updating...");
      return;
    }
    setPhase("idle");
    btn.setAttribute("title", "Up to date");
    window.setTimeout(() => {
      if (btn.isConnected) {
        btn.setAttribute("title", "Check for updates");
        btn.disabled = false;
        btn.classList.remove("busy");
      }
    }, 2000);
  } catch (err) {
    console.error("check_for_updates failed", err);
    btn.setAttribute("title", "Check failed");
    window.setTimeout(() => {
      if (!btn.isConnected) return;
      btn.disabled = false;
      btn.classList.remove("busy");
      if (phaseAtClick === "ready" && version) {
        setPhase("ready", version);
      } else {
        btn.setAttribute("title", "Check for updates");
      }
    }, 2000);
  }
}
