import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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
  opts: { onSettings: () => void; onHide: () => void },
): void {
  root.innerHTML = `
    <div class="header" data-tauri-drag-region>
      <div class="title">Usage</div>
      <div class="header-actions">
        <button type="button" class="icon-btn" id="btn-update" title="Check for updates" aria-label="Check for updates">↻</button>
        <button type="button" class="icon-btn" id="btn-settings" title="Settings" aria-label="Settings">⚙</button>
        <button type="button" class="icon-btn" id="btn-hide" title="Hide" aria-label="Hide">–</button>
      </div>
    </div>
  `;

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
