# Edge Notch Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. 사용자가 별도로 요청하지 않은 병렬 에이전트 작업은 시작하지 않는다.

**Goal:** 화면 가장자리와 곡선으로 이어지는 노치, 공급자 링, 안쪽으로 펼쳐지는 상세를 Windows TokenUsage 기본 UI로 제공한다.

**Architecture:** Rust가 네이티브 배치와 저장을 담당하고 TypeScript가 표시와 상호작용을 담당한다. 기존 OAuth snapshot을 재사용한다. 첫 단계에서 Windows 투명도와 입력 처리를 검증해 창 구성을 확정한다.

**Tech Stack:** 기존 Tauri 2 / Rust / windows crate / vanilla TypeScript / CSS / SVG / Vitest. 새 패키지는 기본 계획에 없다.

**Spec:** [노치 전환 설계안](../specs/2026-09-30-edge-notch-design.md)

상태: 사용자 승인 후 구현과 검증을 수행했다. 아래 체크리스트는 최초 작업 순서이며, 최종 결과와 미검증 환경은 문서 끝 Execution record를 따른다. 커밋과 릴리스는 수행하지 않았다.

## Global Constraints

- Windows 우선, 네 방향 부착까지 이번 범위. 기본 오른쪽 중앙, 링 상시 표시.
- refresh 5초 고정, 직접 OAuth only, 알림·AGY·로컬 사용량 추정 추가 없음.
- 상세 바는 기존 6px / 숫자 2.9em / gutter 2px 및 format/위험도 함수 재사용.
- 애니메이션 160ms, 닫힘 지연 180ms, reduced-motion 즉시. 타이머 테스트는 fake clock.
- 크기 초안: 깊이 72 DIP, 링 40 DIP/4 DIP stroke, 셀 72 DIP, 양끝 여유 각 56 DIP(연결 곡선 포함).
- 구현 전 ENGINEERING.md 재확인. 커밋을 수행할 경우 RELEASE.md를 파일로 열고 요청 범위만 stage.

## Review Focus

- 다른 프로세스 위의 투명 영역이 클릭을 먹는 문제: Task 1 네이티브 시험.
- mixed DPI/음수 좌표 및 모니터 제거: Task 2 순수 계산 테스트 + Task 6 실기.
- 상세 왕복 중 닫힘, hover focus 탈취, 오래된 비동기 resize 결과: Task 3/4 검증.
- 정상처럼 보이는 결측·주간 한도 은폐: Task 4 fixture 검증.
- 이전 설정 손실·프로그램 resize가 배치를 덮어씀: Task 2/5 저장 회귀 테스트.

## Task 1 — 노치 윤곽과 Windows 입력 처리 검증

**Files:** `src-tauri/src/infrastructure/window_ctl.rs`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `src/styles/app.css`; 검증 결과는 위 설계 문서에 기록. 임시 검증 변경은 제품 UI와 혼재하지 않게 구현 단계의 격리 작업 공간에서 수행.

**Interfaces:** 현재 main 창 및 windows crate 사용. 산출물은 검증된 단일/이중 창 결정, 필요한 Win32 호출과 capability 목록이다.

- [ ] 실제 main 창에 오른쪽 곡선 노치와 안쪽 상세의 최소 형상을 그린다. 데이터는 합성 fixture를 사용한다.
- [ ] 기존 강제 최소 크기·DWM ROUND·resize clamp를 노치 경로에서 해제해 프레임과 클립 상태를 확인한다.
- [ ] 뒤에 다른 프로세스의 앱을 두고 투명 곡선 모서리/상세 빈 영역 클릭, 링 클릭, 상세 왕복, hover 비활성 표시를 시험한다.
- [ ] 100/150% 배율 캡처와 실패 조건을 기록한다. 단일 창 실패 시 두 창 방식으로 같은 시험을 반복한다. 필요한 새 crate가 생기면 이유를 먼저 밝힌다.
- [ ] 통과한 방식과 창 수명주기를 설계와 후속 작업 인터페이스에 반영한다. 통과 전 다른 기능 구현을 확장하지 않는다.

**성공:** 사각 프레임 없음, 바깥 접점 고정, 다른 앱 클릭 통과, hover 포커스 유지. HTML 미리보기만으로 통과 처리하지 않는다.

## Task 2 — 배치 모델, 좌표 계산, 설정 호환

**Files:** 새 `src-tauri/src/domain/notch.rs`; 기존 `domain/mod.rs`, `domain/types.rs`, `domain/constants.rs`, `infrastructure/store.rs`, `application/service.rs`, `src/ui/types.ts`; 기존 `src-tauri/tests/risk_scenarios.rs`.

**Interfaces:** `NotchEdge { Top, Right, Bottom, Left }`; `NotchPlacement { edge, monitor_hint: Option<String>, offset: f64 }`를 AppSettings의 serde default 필드로 추가한다. `offset`은 가용 이동 길이의 0..1 비율. `calculate_notch_bounds`는 모니터 물리 경계·작업 영역·scale·placement·콘텐츠 DIP 크기를 입력받아 노치/상세 물리 경계 또는 명시적 배치 오류를 반환하는 순수 함수로 만든다. 구체 구조체 필드는 Task 1의 창 수에 맞춰 확정한다.

- [ ] 네 edge 접점, 음수 x, scale 1/1.25/1.5/2, offset 0/0.5/1, oversized 상세, taskbar 차단 테스트를 먼저 작성하고 실패 확인: `cargo test --manifest-path src-tauri/Cargo.toml --lib notch`.
- [ ] 중앙 배치 사례에서 중심 일치, right edge에서 `x + width == monitor.right`, top edge에서 `y == monitor.top`을 검증한다. 반올림 오차는 물리 1px 이내.
- [ ] offset 범위/유한값 검증과 순수 계산을 구현한다. app service가 placement 저장을 담당하고 command에 정책을 넣지 않는다.
- [ ] 구버전 JSON에 notch 필드 없이 로드해 오른쪽/0.5가 되며 공급자·opacity·hotkey·autostart가 보존되는 회귀 테스트를 추가한다. 잘못된 새 필드는 에러를 드러내며 전체 설정을 조용히 초기화하지 않는다.
- [ ] 위 테스트와 `cargo test --manifest-path src-tauri/Cargo.toml --test risk_scenarios` 통과를 확인한다.

**성공:** 창 없이 배치 정책을 검증할 수 있고 기존 설정이 손실 없이 열린다.

## Task 3 — 네이티브 배치 제어와 기존 resize 경로 교체

**Files:** 새 `src-tauri/src/infrastructure/notch_window.rs`; 기존 `infrastructure/mod.rs`, `window_ctl.rs`, `commands.rs`, `lib.rs`, `state.rs`, `infrastructure/tray.rs`, `tauri.conf.json`, `capabilities/default.json`; `src/ui/app.ts`, `content-size.ts`, `window-nudge.ts`.

**Interfaces:** `set_notch_placement(placement)`는 검증·저장·재배치를 요청한다. `set_notch_surface(request)`는 선택된 surface(none/detail/settings), 측정한 DIP 크기, 증가하는 revision을 받아 기존보다 오래된 요청을 무시한다. Rust controller가 최신 요청을 직렬 적용한다. 두 창 채택 시 frontend별 이벤트/권한을 Task 1에서 문서화한다.

- [ ] 배치 요청 순서 역전, 같은 크기 반복, UI hide 중 늦게 도착한 detail 요청 테스트를 작성해 실패를 확인한다.
- [ ] Task 2 계산을 사용해 위치/크기를 적용한다. 모니터 변경/scale 변경/절전 복귀 및 명시적 설정 변경에 재계산한다. native 오류는 로그/명령 실패로 보고한다.
- [ ] 기존 content-hug, 강제 최소 크기, onMoved/onResized geometry 저장이 노치 요청과 경쟁하지 않게 제거 또는 해당 legacy 경로를 종료한다. 이번 변경으로 미사용이 된 코드만 제거한다.
- [ ] 트레이/단축키 show/hide를 노치 controller로 연결한다. hover 상세에는 set_focus를 호출하지 않는다. 키보드 접근은 명시적 호출 경로로 제공한다.
- [ ] 단위 테스트 및 실제 Windows에서 연속 펼침/접힘 20회, 빠른 공급자 전환, hide/show 후 위치 유지 확인.

**성공:** 붙어 있는 쪽은 움직이지 않고 안쪽으로만 확장하며 resize feedback loop와 설정 쓰기 반복이 없다.

## Task 4 — 링과 상세, 상태 전이

**Files:** 새 `src/ui/notch.ts`, `src/ui/notch-state.ts`, `src/ui/notch-state.test.ts`, `src/ui/notch.test.ts`, `src/styles/notch.css`; 기존 `providers.ts`, `format.ts`, `types.ts`, `app.ts`, `src/main.ts`, `styles/tokens.css`; 로고는 `src/assets/`에 출처/허가 확인 후 추가.

**Interfaces:** `mountNotch(root, callbacks)`는 snapshot 갱신 및 dispose를 제공한다. 순수 `reduceNotchState(state, event)`가 hovered/focused/pinned provider와 settings 상태를 결정한다. DOM controller가 fake-clock으로 검증 가능한 180ms 닫힘 예약을 관리하고 surface 요청을 Task 3으로 보낸다.

- [ ] fixture로 정상 0/73/100/125%, primary null, auth_required, degraded, primary 10% + weekly critical을 작성한다. primary 의미는 adapter/snapshot 선택 구현을 읽고 고정한다.
- [ ] reducer 테스트: 노치→상세 이동 유지, 재진입 취소, 고정 후 leave 유지, provider 제거 시 닫힘, Escape 우선순위, 설정 중 자동 닫힘 금지를 작성하고 `npx vitest run src/ui/notch-state.test.ts src/ui/notch.test.ts`로 실패 확인.
- [ ] 곡선 silhouette와 SVG 링을 렌더링한다. 기존 clampPct/formatPct/levelClass 및 상세 바 formatter를 재사용한다. null은 0%가 아니며 125%는 꽉 찬 링 + 125% 텍스트가 된다.
- [ ] DOM 전체를 매 refresh마다 교체해 focus/hover가 유실되지 않도록 provider key별 갱신한다. request payload에 raw OAuth 응답/자격증명을 추가하지 않는다.
- [ ] 테스트 통과 후 실제 창에서 모든 fixture, 세로/가로, reduced-motion, 키보드 Tab/Enter/Escape와 스크린리더 label을 확인한다.

**성공:** 스크린샷의 노치 형태와 정보 계층을 구현하고 주간 위험·결측이 은폐되지 않는다.

## Task 5 — 설정 이동, 배치 조작, 문서 동기화

**Files:** 기존 `settings-panel.ts`, `header.ts`, `app.ts`, `window-nudge.ts`, `window-nudge.test.ts`, `opacity.ts`, `styles/app.css`; `AGENTS.md`, `PRODUCT.md`, `DESIGN.md`, `docs/ARCHITECTURE.md`, `docs/windows-dev.md`, `docs/testing.md`.

**Interfaces:** 기존 설정 command 유지. Edge/Monitor/Recentre만 Task 3 placement command로 연결한다. 설정용 별도 공급자 조회 루프를 만들지 않는다.

- [ ] 드래그 완료 때만 placement 저장, 방향키가 선택 edge 축에서만 이동, 없는 모니터 복원, settings overflow 테스트를 추가한다.
- [ ] 기존 header 기능을 노치 설정으로 옮기고 Edge/Monitor/Recentre와 드래그 핸들을 구현한다. last-provider lock, Copy Log, Quit, 업데이트, version, autostart를 유지한다.
- [ ] opacity 최솟값에서 본문/기간/리셋 가독성과 링 위험색을 실제 배경 위에서 확인한다. 기존 card tint 값은 보존하고 상세 배경에만 적용한다.
- [ ] 새 규칙으로 문서를 함께 수정한다. 네이티브 노치에는 8px DWM 규칙을 강제하지 않고 상세 바의 고정 열 규칙은 유지한다고 명시한다.
- [ ] 신규 설정과 과거 설정 파일 각각으로 재시작해 상태 복원 확인.

**성공:** 기존 필수 기능 누락 없이 네 방향과 모니터를 선택·이동·복원할 수 있다.

## Task 6 — 통합 검증과 시각 검수

- [ ] `npm test` 실행: Vitest, Rust lib, risk_scenarios 모두 통과.
- [ ] `npm run build` 실행: TypeScript/Vite 성공.
- [ ] `npm run tauri dev`로 Windows 실물 실행. 기본 right, left, top, bottom의 펼침/닫힘 캡처 저장.
- [ ] 100/125/150/200% 배율, 두 모니터의 서로 다른 배율 및 음수 좌표, 모니터 제거, 해상도 변경, 절전 복귀 검증. 없는 환경은 미검증으로 명시하고 통과라 보고하지 않는다.
- [ ] taskbar 위치/자동 숨김, 최대화 창 위, 전체 화면 전환, 투명 영역 클릭 통과, hover 포커스, 고정 상세와 키보드 조작 확인.
- [ ] 공급자 1/2/3개, 긴 문구, 오류와 결측, 최소 opacity, reduced-motion으로 디자인 비교. 문서의 시작 치수를 실물 결과에 맞춰 확정한다.
- [ ] diff를 검토해 공급자 조회 정책·debug autostart·서명키가 변경되지 않았고 노치 요구 외 작업이 없는지 확인한다. 구현 결과와 캡처/검증 한계를 보고한다. 배포는 별도 요청 범위다.

**성공:** 설계안 완료 기준 전부 충족. 테스트/빌드 성공과 Windows 시각·입력 검증을 구분해 보고한다.

## Execution record - 2026-10-01

- Tasks 1-5 implemented in `feat/edge-notch`; single native transparent window, OS hit testing, atomic Windows move+resize, shared Rust-to-CSS layout metrics.
- All four review findings corrected: failed drag-save release, secondary warning, settings retained on provider removal, rounded-detail click-through. Reviewer recheck found no remaining important errors in those fixes.
- User's reference-image correction: 56 DIP padding at both ends, also left/right in horizontal orientation, independent of provider count.
- Automated suite: 32 Vitest + 31 Rust library + 11 risk + 8 notch tests; frontend production build passed.
- Real WebView2 smoke and regression passed on 100% Windows displays; scripts in `scripts/notch-smoke.py` and `scripts/notch-regression.py`.
- Task 6 environment limits: native 125/150/200% and mixed DPI, physical monitor unplug, sleep/resume and auto-hide taskbar not exercised. Geometry and hit logic at all four scales is unit-tested. Do not mark these native cases passed.
- No vendor source, refresh policy, release updater or credential behavior changed. Existing installed binary is not replaced by the preview.

## Accepted visual and interaction revisions

The user accepted the final native screenshots after these refinements, superseding
initial dimensions and controls above: 72 DIP depth, 104 DIP cells, 50 DIP end
insets, and two tangent 36 DIP circular arcs per end. Drag anywhere on the notch;
right-click or Shift+F10 opens settings. No separate move/settings buttons.
Final automated validation: 32 Vitest + 31 Rust library + 11 risk + 9 notch = 83
passing tests; npm run build passed. Native screenshots and gesture smoke were
reviewed separately. Existing environment limitations still apply.
