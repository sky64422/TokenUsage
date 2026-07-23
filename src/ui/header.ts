import { invoke } from "@tauri-apps/api/core";

export function renderHeader(
  root: HTMLElement,
  opts: { onSettings: () => void; onRefresh: () => void; onHide: () => void },
): void {
  root.innerHTML = `
    <div class="header">
      <div class="title">Usage</div>
      <div class="header-actions">
        <button class="icon-btn" id="btn-refresh" title="Refresh" type="button" aria-label="Refresh">↻</button>
        <button class="icon-btn" id="btn-settings" title="Settings" type="button" aria-label="Settings">⚙</button>
        <button class="icon-btn" id="btn-hide" title="Hide" type="button" aria-label="Hide">–</button>
      </div>
    </div>
  `;
  root.querySelector("#btn-settings")?.addEventListener("click", opts.onSettings);
  root.querySelector("#btn-refresh")?.addEventListener("click", opts.onRefresh);
  root.querySelector("#btn-hide")?.addEventListener("click", opts.onHide);

  root.querySelector(".title")?.addEventListener("dblclick", () => {
    void invoke<boolean>("check_for_updates").catch(() => undefined);
  });
}

export function setSettingsButtonActive(root: HTMLElement, active: boolean): void {
  root.querySelector("#btn-settings")?.classList.toggle("active", active);
}
