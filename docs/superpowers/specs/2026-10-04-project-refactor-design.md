# TokenUsage 전면 리팩터링 설계안

작성: 2026-10-04. 상태: 사용자 승인 후 구현·검증 완료. 결과는 구현 계획의 검증 기록 참조.

## 목표와 해석

사용자 요청은 프로젝트 전면 리팩터링이다. 기본 해석은 기존 제품 동작과
외부 인터페이스를 유지하면서 프런트엔드와 Rust 양쪽의 책임 경계와 테스트
가능성을 개선하는 것이다. UI 재디자인이나 기능 추가는 목표에 포함하지 않는다.

성공 기준은 파일 수나 줄 수 감소가 아니다. 드래그, 네이티브 표면 요청,
업데이트 상태, 활동 기록 해석을 각각 관련 없는 부수 효과 없이 검증할 수 있어야
하며, 중복된 공급자 정보와 갱신 경로는 하나의 구현을 사용해야 한다.

## 현재 확인한 기준 상태

- 브랜치 `main`, 시작 커밋 `cdbcc21`, 조사 시작 시 작업 트리 깨끗함.
- `npm test`: 프런트엔드 38, Rust 단위 53, risk 14, notch 21 통과.
  총 126 통과, 실제 AGY CLI를 호출하는 opt-in 테스트 1개 제외.
- `npm run build`: TypeScript 및 Vite 빌드 통과.
- 이번 조사에서 네이티브 UI, 실제 공급자 API, 설치 및 업데이터는 실행 검증하지 않았다.

## 코드에서 확인한 개선 지점

| 위치 | 현재 책임/중복 | 변경 목적 |
| --- | --- | --- |
| `src/ui/app.ts` | 앱 조립, 설정 콜백, 화면 표시, 표면 요청 큐, 드래그 세션과 DOM capture | 앱 조립에서 드래그 및 표면 요청 수명주기 분리 |
| `src/ui/settings-panel.ts` | 탭/설정 표시, 저장 처리, 업데이트 IPC 및 상태 전이 | 설정 화면과 업데이트 제어 책임 분리 |
| `src/ui/notch.ts`, `settings-panel.ts` | 공급자 SVG 매핑 중복 | 한 공급자 표현 정보를 여러 화면에서 재사용 |
| `src-tauri/src/lib.rs`, `commands.rs`, `infrastructure/poll.rs` | `spawn_blocking(refresh_all)` 및 `snapshots-updated` 발행 반복 | 갱신 실행과 실패 보고를 같은 경로로 유지 |
| `application/service.rs`, `infrastructure/notch_window.rs` | 공급자 설정 접근 및 활성 공급자 계산 반복 | 기존 `AppSettings`에서 같은 순서와 활성 정책 제공 |
| `infrastructure/activity.rs` | Tauri 이벤트, 주기 실행, 파일 탐색/읽기, 공급자 기록 해석, TTL, AGY 감지 | 기록 해석과 파일/런타임 수명주기 분리 |
| `domain/notch.rs` | 정적 레이아웃/히트 테스트와 드래그 경계 전환 | 레이아웃 수학과 드래그 정책의 독립 검증 |

이미 분리된 quota HTTP 캐시/상태 매핑과 snapshot 계산은 재사용한다.
공급자별 OAuth 차이를 범용 어댑터 하나로 통합하지 않는다.
`commands.rs`는 상당 부분 얇은 위임이므로 모든 command를 다시 작성할 이유가 없다.

## 접근 방식 비교

1. **책임별 단계적 리팩터링 (권장):** 현재 Tauri/TypeScript 구조와 계약을 유지하고
   관련 책임만 옮긴다. 각 단계에서 회귀를 찾기 쉽고 별도로 되돌릴 수 있다.
2. **프런트엔드 한정 정리:** 변경 위험과 양은 적지만 Rust 갱신 중복과 활동 감지
   결합은 남아 전면 리팩터링 요청을 충분히 해결하지 못한다.
3. **프레임워크/런타임 전면 재작성:** UI 및 공급자 네 계열을 다시 검증해야 한다.
   현재 테스트와 빌드가 통과하고 있어 재작성 비용을 정당화할 근거가 없다.

## 유지할 계약

- Tauri command 이름, payload, 이벤트 이름 및 persisted JSON 스키마 유지.
- 네 공급자의 순서, 사용자 표시/숨김 설정, 최소 한 공급자 표시 정책 유지.
- Rust가 물리적 위치와 hit-test를 소유한다. 고정 native canvas, 64 DIP 깊이,
  72 DIP 셀, 44 DIP inset 및 네 방향 배치/음수 모니터 좌표를 유지한다.
- 드래그 preview는 저장하지 않는다. 완료/취소, 오래된 세션 무시, pointer capture
  정리 및 클릭 억제의 기존 순서를 유지한다.
- hover/pin 구분, 설정 3탭, opacity 가독성, 공급자별 상세 높이와 fixed columns 유지.
- 주기 갱신 5초, AGY background 완료의 즉시 알림 및 디스크 캐시 유지.
- Grok의 누락된 percentage는 unknown, 명시적 0만 0%. `productUsage`를 표시하지 않는다.
- 공급자별 HTTP timeout, cache TTL, 인증/refresh payload 유지. 현재 소스에서
  Claude/Codex body cache는 45초, Grok은 15초다. 문서의 일괄 45초 설명을
  구현의 기준으로 삼지 않는다.
- 활동은 quota와 별개다. 기록 본문/프롬프트/자격 증명을 새로 보관하거나 출력하지 않는다.
- debug 실행에서 OS autostart enable을 호출하지 않는다.

## 단계별 설계와 통과 기준

### 1. 공급자 표현 및 설정 접근

프런트엔드의 기존 provider ID/type은 유지하고, 여러 화면이 쓰는 공급자 이름과
mark 정보를 한 모듈에서 제공한다. Rust는 기존 `AppSettings`에 공급자 접근 및
활성 목록 계산을 모으고 서비스와 네이티브 창이 이를 사용하게 한다.

통과 기준: 공급자 표시 순서와 마지막 공급자 잠금이 동일하고, 설정 JSON
round-trip 및 기존 risk 테스트가 통과한다. 새 의존성은 추가하지 않는다.

### 2. 프런트엔드 노치 제어

`app.ts`에는 앱 조립과 snapshot/activity/interaction 연결을 남긴다.
드래그 제어는 별도 책임으로 옮겨 세션 ID, capture, begin/finish 직렬화,
늦은 이벤트 무시, 클릭 억제를 소유한다. 표면 제어는 별도 책임으로 옮겨
요청 key, revision, 직렬 실행과 최신 응답 적용을 소유한다.

DOM 크기 측정은 UI에 남기고 물리적 위치 계산은 기존 Rust 구현에 남긴다.
단순히 `invoke()`를 한 번 감싸는 함수들을 늘리지 않고, 명확한 상태 수명주기를
가진 제어 단위만 만든다. 각 제어 단위는 자신이 등록한 이벤트를 해제할 수 있어야 한다.

통과 기준: 지연된 표면 응답, begin 실패, 취소, 오래된 drag completion을
주입 가능한 비동기 응답으로 검증한다. 기존 pin/hover/Escape 테스트와 실제
Windows preview의 드래그 및 hover 안정성 검증이 통과해야 한다.

### 3. 설정 화면과 업데이트 제어

설정 패널은 탭, 컨트롤, 상태 표시를 담당한다. 업데이트 제어는 command 실행,
available/progress/ready/failed 이벤트, busy 상태 및 재시작 동작을 소유한다.
이벤트 등록이 비동기로 완료되는 경우도 정리 가능한 수명주기를 명시한다.
기존 사용자 문구, 버튼 크기 및 CSS는 유지한다.

통과 기준: ready 후 늦은 progress, 실패 후 retry, 중복 클릭, 종료 시 구독 정리를
검증한다. `scripts/settings-smoke.py`의 저장 실패 복원, pending controls,
마지막 공급자 잠금, updater 상태 검증이 통과한다. 실제 업데이트는 설치하지 않는다.

### 4. Rust 갱신 실행과 앱 연결

`infrastructure/poll.rs`의 기존 갱신 역할을 확장하여 시작/주기/창 다시 표시에서
같은 blocking refresh 및 event 발행 경로를 사용한다. 갱신 주기, AGY 알림,
숨김 상태 정책 및 초기 표시 타이밍은 각각의 호출 조건으로 유지한다.
UI command는 입력/결과 변환과 서비스 호출만 담당한다.

통과 기준: 현재 visibility/risk 테스트와 모든 provider parser 테스트가 통과하고,
초기 갱신·숨김·재표시 및 AGY 알림 경로를 fixture로 검증한다. Thread join 또는
event 발행 오류를 성공 결과로 숨기지 않고 기존 diagnostics 경로로 보고한다.

### 5. 활동 감지와 노치 도메인 경계

활동 모듈은 기존 public entrypoint를 유지하며 내부를 런타임/event 연결,
bounded 파일 수집, evidence 해석으로 분리한다. Grok `turn_ended`의 quota cache
무효화는 해석 결과를 받은 adapter 쪽에서 실행하도록 하여 parser가 quota
전송 계층을 직접 호출하지 않게 한다. 기존에 받아들인 레코드의 순서와 무효화
조건을 유지한다. AGY 파일 기반 감지는 별도의 현재 책임으로 유지한다.

노치 도메인에서는 드래그 전환 정책을 별도 모듈로 옮기고 기존 공개 경로는
재노출하여 호출부와 테스트 계약을 유지한다. 고정 canvas 계산과 곡선 hit-test는
현재 응집된 알고리즘을 그대로 둔다. native controller의 lock/타이머/Win32 호출
순서를 파일 크기만을 이유로 변경하지 않는다.

통과 기준: malformed/partial/future/expired evidence, 같은 시각의 완료 레코드,
여러 세션 집계, preview read 차단 테스트가 통과한다. 모든 geometry/drag 테스트,
특히 DPI·모니터 seam·상세 높이 변경 시 canvas 고정 회귀가 통과한다.

### 6. 통합 검증 및 문서

변경된 책임 경계와 실제 명령/테스트 위치를 `docs/ARCHITECTURE.md`,
`docs/providers.md`, `docs/testing.md`에 반영한다. 제품 디자인 계약은 유지한다.

필수 자동 검증:

```text
npm test
npm run build
cd src-tauri
cargo clippy --all-targets -- -D warnings
```

프런트엔드 검증은 기존 browser smoke를 사용한다. 네이티브 검증은
`docs/windows-dev.md`의 별도 preview identifier, 별도 WebView profile,
`TOKENUSAGE_SKIP_DIRECT_QUOTA=1`을 사용한다. 실제 사용자 설정을 fixture로 덮어쓰지 않는다.

대상 smoke: settings, Grok unknown/zero, detail layout, native notch regression,
edge/seam drag, hover stability. 기존 smoke 실패는 baseline과 비교하여 이번 회귀인지
구분한다. 혼합 DPI 실물 모니터, unplug/replug 및 실제 공급자 인증/업데이트 설치는
실행한 범위만 결과에 기록한다.

## 구조 변경과 별도로 다룰 관찰

일반 설정 `mutate_settings()`와 `set_provider_enabled()`는 메모리를 먼저 변경하고
저장한다. 반면 `set_notch_placement()`는 저장 성공 후 메모리를 확정한다.
따라서 일반 설정 저장 실패에서 메모리와 UI rollback이 어긋날 가능성이 있다.
현재 자동 테스트 통과만으로 이 실패 경로가 안전하다고 볼 수 없다.

이 문제는 별도 실패 재현 테스트로 확인하고, 수정한다면 동작 보존 리팩터링과
구분된 변경으로 다룬다. 공급자 갱신 동시성이나 캐시 정책도 조사만으로
새 정책을 도입하지 않는다.

## 검토 항목

권장 범위는 1~6 전체다. 사용자 우선순위 답변이 없다면 유지보수성 중심으로
해석한다. 설계 검토 후 구현 계획에 정확한 이동 파일과 테스트 순서를 기록한다.
전체 완료는 각 단계의 코드와 검증 결과가 갖춰졌을 때만 보고한다.
