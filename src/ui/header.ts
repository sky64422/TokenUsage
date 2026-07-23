import { invoke } from "@tauri-apps/api/core";

export function renderHeader(
  root: HTMLElement,
  opts: { onSettings: () => void; onRefresh: () => void; onHide: () => void },
): void {
  root.innerHTML = `
    <div class="header">
      <div class="title">Token Usage</div>
      <div class="header-actions">
        <button class="icon-btn" id="btn-update" title="Check for updates" type="button" aria-label="Check for updates">⬆</button>
        <button class="icon-btn" id="btn-refresh" title="Refresh usage" type="button" aria-label="Refresh usage">↻</button>
        <button class="icon-btn" id="btn-settings" title="Settings" type="button">⚙</button>
        <button class="icon-btn" id="btn-hide" title="Hide" type="button">–</button>
      </div>
    </div>
  `;
  root.querySelector("#btn-settings")?.addEventListener("click", opts.onSettings);
  root.querySelector("#btn-refresh")?.addEventListener("click", opts.onRefresh);
  root.querySelector("#btn-hide")?.addEventListener("click", opts.onHide);

  const updateBtn = root.querySelector("#btn-update") as HTMLButtonElement | null;
  updateBtn?.addEventListener("click", (e) => {
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
  } catch (err) {
    console.error("check_for_updates failed", err);
    btn.setAttribute("title", "Check failed");
  }
  window.setTimeout(() => {
    if (btn.isConnected) {
      btn.setAttribute("title", originalTitle);
      btn.disabled = false;
      btn.classList.remove("busy");
    }
  }, 2000);
}
