import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/app.css";
import "./styles/notch.css";
import { mountApp } from "./ui/app";

window.addEventListener("DOMContentLoaded", () => {
  const root = document.querySelector("#app");
  if (!root) {
    console.error("#app root missing");
    return;
  }
  void mountApp(root as HTMLElement).catch((err) => {
    console.error("Failed to mount app", err);
    const message = document.createElement("div");
    message.className = "startup-error";
    message.textContent = `Failed to load: ${String(err)}`;
    root.replaceChildren(message);
  });
});
