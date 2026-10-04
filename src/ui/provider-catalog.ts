import claude from "../assets/marks/claude.svg";
import codex from "../assets/marks/codex.svg";
import grok from "../assets/marks/grok.svg";
import agy from "../assets/marks/agy.svg";
import type { ProviderId } from "./types";

export const PROVIDER_CATALOG: Record<ProviderId, { label: string; mark: string }> = {
  claude: { label: "Claude", mark: claude },
  codex: { label: "Codex", mark: codex },
  grok: { label: "Grok", mark: grok },
  agy: { label: "Antigravity", mark: agy },
};
