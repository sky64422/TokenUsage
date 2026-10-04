export type UpdatePhase = "idle" | "downloading" | "ready";

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

export interface UpdateState {
  readonly phase: UpdatePhase;
  readonly version: string | null;
  readonly hint: string;
  readonly busy: boolean;
  readonly error: string;
  readonly checked: boolean;
}

export interface UpdateDependencies {
  check: () => Promise<boolean>;
  listen: <T>(event: string, handler: (event: { payload: T }) => void) => Promise<() => void>;
  schedule: (callback: () => void, delay: number) => () => void;
  reportError: (message: string, error: unknown) => void;
}

export const UPDATE_DOWNLOADING = "다운로드 중…";
export const UPDATE_CHECKING = "확인 중…";
export const UPDATE_READY = "준비 완료";
export const UPDATE_FAILED = "업데이트 실패";
export const UPDATE_RESTARTING = "재시작 중…";
const UPDATE_CURRENT = "최신 버전";
const CHECK_FEEDBACK_MS = 2500;
const MAX_DOWNLOAD_PROGRESS = 99;

/** Owns updater IPC, event subscriptions and transient feedback, independently of the view. */
export function createUpdateController(deps: UpdateDependencies) {
  let state: UpdateState = { phase: "idle", version: null, hint: "", busy: false, error: "", checked: false };
  let disposed = false;
  let actionPending = false;
  let cancelFeedback: (() => void) | undefined;
  const subscribers = new Set<(state: UpdateState) => void>();
  const unlisteners: Array<() => void> = [];

  function update(changes: Partial<UpdateState>) {
    if (disposed) return;
    state = { ...state, ...changes };
    for (const subscriber of subscribers) subscriber(state);
  }

  function subscribeEvent<T>(event: string, handler: (payload: T) => void) {
    void deps.listen<T>(event, ({ payload }) => {
      if (!disposed) handler(payload);
    }).then(unlisten => {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    }).catch(error => deps.reportError(`${event} subscription failed`, error));
  }

  subscribeEvent<UpdateInfo>("update-available", payload => {
    if (!payload?.version) return;
    update({ phase: "downloading", error: "", version: payload.version, busy: false, hint: UPDATE_DOWNLOADING });
  });
  subscribeEvent<DownloadProgress>("update-download-progress", payload => {
    if (!payload?.version || state.phase === "ready") return;
    const hint = payload.content_length && payload.content_length > 0
      ? `${Math.min(MAX_DOWNLOAD_PROGRESS, Math.round((payload.received / payload.content_length) * 100))}% 다운로드`
      : UPDATE_DOWNLOADING;
    update({ phase: "downloading", version: payload.version, busy: false, hint });
  });
  subscribeEvent<UpdateInfo>("update-ready", payload => {
    if (!payload?.version) return;
    update({ phase: "ready", error: "", version: payload.version, busy: false, hint: UPDATE_READY });
  });
  subscribeEvent<unknown>("update-not-available", () => {
    update({ phase: "idle", version: null, busy: false, checked: true, error: "", hint: UPDATE_CURRENT });
  });
  subscribeEvent<unknown>("update-failed", payload => {
    update({ busy: false, phase: state.phase === "ready" ? "ready" : "idle", error: typeof payload === "string" ? payload : UPDATE_FAILED });
  });

  async function action(): Promise<void> {
    if (disposed || actionPending) return;
    const { phase: phaseAtClick, version } = state;
    if (phaseAtClick === "downloading") {
      update({ hint: UPDATE_DOWNLOADING });
      return;
    }
    actionPending = true;
    cancelFeedback?.();
    cancelFeedback = undefined;
    update({ busy: true, checked: false, error: "", hint: phaseAtClick === "ready"
      ? version ? `${version} 설치를 위해 재시작…` : UPDATE_RESTARTING
      : UPDATE_CHECKING });
    try {
      const hasUpdate = await deps.check();
      if (disposed) return;
      if (hasUpdate) {
        update({ busy: false, phase: state.phase === "idle" ? "downloading" : state.phase,
          hint: state.phase === "ready" ? state.hint : UPDATE_DOWNLOADING });
        return;
      }
      update({ phase: "idle", version: null, busy: false, hint: UPDATE_CURRENT, checked: true });
      cancelFeedback = deps.schedule(() => {
        cancelFeedback = undefined;
        if (state.phase === "idle") update({ checked: false });
      }, CHECK_FEEDBACK_MS);
    } catch (error) {
      if (disposed) return;
      deps.reportError("check_for_updates failed", error);
      update({ busy: false, error: formatUpdateError(error),
        ...(phaseAtClick === "ready" && version ? { phase: "ready", version } : {}) });
    } finally {
      actionPending = false;
    }
  }

  return {
    subscribe(subscriber: (state: UpdateState) => void) {
      if (!disposed) {
        subscribers.add(subscriber);
        subscriber(state);
      }
      return () => { subscribers.delete(subscriber); };
    },
    action,
    destroy() {
      if (disposed) return;
      disposed = true;
      cancelFeedback?.();
      subscribers.clear();
      for (const unlisten of unlisteners) unlisten();
      unlisteners.length = 0;
    },
  };
}

function formatUpdateError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) return String(error.message);
  return "업데이트 확인 실패";
}
