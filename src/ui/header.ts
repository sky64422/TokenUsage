import { invoke } from "@tauri-apps/api/core";

export function renderHeader(
  root: HTMLElement,
  opts: { onSettings: () => void; onHide: () => void },
): void {
  // Chrome aligned with WarRoom: ↻ = app update, not data refresh.
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
  updateBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    void checkForUpdates(updateBtn);
  });
}

export function setSettingsButtonActive(root: HTMLElement, active: boolean): void {
  root.querySelector("#btn-settings")?.classList.toggle("active", active);
}

async function checkForUpdates(btn: HTMLButtonElement): Promise<void> {
  const originalTitle = btn.getAttribute("title") ?? "Check for updates";
  btn.disabled = true;
  btn.classList.add("busy");
  btn.setAttribute("title", "Checking...");
  try {
    const hasUpdate = await invoke<boolean>("check_for_updates");
    btn.setAttribute("title", hasUpdate ? "Updating..." : "Up to date");
    window.setTimeout(() => {
      if (btn.isConnected) {
        btn.setAttribute("title", originalTitle);
        btn.disabled = false;
        btn.classList.remove("busy");
      }
    }, 2000);
  } catch (err) {
    console.error("check_for_updates failed", err);
    btn.setAttribute("title", "Check failed");
    window.setTimeout(() => {
      if (btn.isConnected) {
        btn.setAttribute("title", originalTitle);
        btn.disabled = false;
        btn.classList.remove("busy");
      }
    }, 2000);
  }
}
