# Liquid 화면 공유 누락: 구조 개선 검토

검토일: 2026-11-01. 최초 설계 검토 후 세션별 제외의 독립 실험까지 진행했다. 제품 캡처 엔진 교체는 아직 하지 않았다. 최신 실험 결과는 마지막 절을 참고한다.

이후 실제 노치 preview를 구현하고 [별도 프로세스·CPU/GPU·복구 추가 검증](liquid-wgc-deep-validation.md)을 수행했다. 현재 배포 판단과 자원 비용은 이 추가 보고서를 참고한다.

## 결론

제품의 최종 조건은 **표시 중인 노치 HWND를 항상 `WDA_NONE`으로 유지하는 것**이다. 자체 배경 획득 때문에 다른 프로그램의 캡처 결과를 바꾸지 않아야 한다. 제외 시간을 줄이는 수정은 완전한 해결책이 아니다.

권장 순서는 다음과 같다.

1. Windows Graphics Capture의 세션별 창 제외 API를 작은 독립 실험으로 검증한다. 공식 지원 범위와 런타임 동작이 확인되면 기존 픽셀 기반 외형을 유지하기 좋은 후보다.
2. 그것이 지원되지 않거나 실험 API에 의존해야 한다면, 같은 HWND의 네이티브 배경 합성을 검증한다. 과거 비활성 창 실패 원인을 재현하고 해소하기 전에는 복원하지 않는다.
3. 어느 경로도 보장되지 않는 환경에서는 캡처 없는 재질을 사용한다. 가독성을 유지하는 틴트·테두리·하이라이트로 표시하며 실시간 뒤쪽 배경 블러의 손실을 명시한다. 실패 시 기존 전역 제외 방식으로 자동 복귀하지 않는다.

두 후보 모두 아직 제품 채택을 확정할 단계는 아니다. 특히 지원하지 않는 OS에서도 기존 외형과 화면 공유를 모두 똑같이 보장한다고 약속할 근거는 없다.

## 확인된 원인과 근거의 범위

[검증 보고서](liquid-validation-report.md)의 OS 스크린샷 20장 중 11장, 재실행 20장 중 8장에서 노치 누락을 확인했다. 이 수치는 해당 실험의 결과이며 모든 화상회의 프로그램의 누락 확률이 아니다.

현재 `src-tauri/src/infrastructure/liquid_glass.rs`의 캡처 명령은 `WDA_EXCLUDEFROMCAPTURE`를 설정하고 `DwmFlush` 후 배경을 획득한다. 프런트엔드의 배치 종료 시 해제는 무기한 제외를 막지만 외부 캡처와의 동시 실행을 원자적으로 묶지 못한다.

Microsoft의 [SetWindowDisplayAffinity 문서](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity)는 이 속성을 최상위 창에 적용하며, `WDA_EXCLUDEFROMCAPTURE`가 캡처에서 창을 제외한다고 설명한다. 호출자별 제외 설정이 아니다. 따라서 해제를 더 일찍 하거나 캡처 주기를 바꿔도 외부 캡처가 제외 구간을 읽을 수 있다는 구조는 남는다.

## 후보 비교

| 방법 | 장점 | 제약 및 판단 |
| --- | --- | --- |
| 제외 시간 단축·`DwmFlush` 조정 | 변경량이 작음 | 충돌 구간만 줄어듦. 최종 해결로 채택하지 않음 |
| 일반 WGC/DXGI로 캡처 엔진만 교체 | GPU 프레임·변경 알림 활용 가능 | 자기 창 제외가 별도로 해결되지 않으면 재귀 캡처가 남음 |
| WGC 세션별 창 제외 | 기존 Canvas·블러·대비 계산 유지 가능성이 높음 | 신규 API의 지원·실험 상태·가림 영역 복원·외부 캡처 독립성 확인 필요 |
| 같은 HWND의 네이티브 배경 합성 | 화면 읽기·CPU 픽셀 복사·Canvas 업로드를 제거할 가능성 | 비활성 창, 커스텀 클립, WebView 적층, 캡처 결과 검증 필요 |
| 캡처 없는 재질 | 자체 캡처 제외와 재귀 캡처가 없음 | 실시간 배경 블러 외형을 완전히 유지하지 못함. 지원 불가 시 대안 |
| 전경·배경을 별도 HWND로 분리 | 전경만 캡처에 남길 수 있는 설계 여지 | 배경 누락·창별 공유·Z-order·DPI·입력·수명주기 복잡도. 우선순위 낮음 |

창을 잠깐 숨기거나 투명도를 0으로 만드는 방식도 다른 캡처의 관측 결과를 바꾸므로 근본 대안이 아니다. 다른 창들을 열거하여 `PrintWindow` 등으로 직접 합성하는 방법은 가려진 GPU 콘텐츠·팝업·보호 콘텐츠까지 OS 합성을 재현해야 하므로 이 제품에 적합하지 않다.

## 후보 A: 세션별 창 제외

공식 문서에서 [IDisplayGraphicsCaptureSession](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.idisplaygraphicscapturesession?view=winrt-26100)와 [SetWindowExclusionList](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.idisplaygraphicscapturesession.setwindowexclusionlist?view=winrt-26100)를 확인했다. 세션에 제외 창 목록을 설정하는 후보 API다. 그러나 메서드 문서에는 사전 출시 경고가 있고 반환값과 동작 보장의 설명도 부족하다. URL의 `view=winrt-26100`만으로 모든 26100 이상 시스템에서 정식 지원된다고 결론 내리지 않는다.

현재 PC는 Windows 빌드 26200이고 설치 SDK Include는 10.0.26100.0이다. 해당 SDK의 `winrt/windows.graphics.capture.h`에는 위 인터페이스·메서드가 없었다. 이는 로컬 헤더 부재의 증거이며 런타임 미지원의 증거는 아니다. 이번 검토에서는 실제 캡처 세션의 QueryInterface를 실행하지 않았다.

독립 실험의 통과 조건:

- 정식 SDK/API 계약과 최소 지원 OS를 확인하고 실제 세션에서 인터페이스 제공 여부를 검사한다. 문서에 없는 ABI를 추측해 제품에 넣지 않는다.
- HWND는 `WDA_NONE`인 채 우리 세션에서만 노치를 제외한다. 노치 자리의 결과가 검은색·빈 영역이 아니라 실제 뒤쪽 창의 최신 픽셀인지 확인한다.
- 동시에 제외 없는 두 번째 캡처 세션과 OS 스크린샷에서 노치가 계속 포함되는지 확인한다.
- 배경이 정지하면 프레임 생산도 안정되는지 확인한다. 노치 자체 애니메이션이 불필요한 프레임을 계속 발생시키는지 측정한다.
- 캡처 표시 테두리·권한·커서 포함·모니터 변경·장치 손실 처리를 검증한다. 제품 동작을 위해 OS의 권한 정책을 우회하지 않는다.

채택 시에는 우선 기존 프레임 포맷과 표면별 좌표를 유지하고 공급원만 교체한다. 전체 모니터 프레임을 받더라도 CPU로 읽는 영역은 rail/detail로 제한하고 오래된 대기 프레임은 버린다. 단, 작은 영역을 잘라 읽는다고 전체 모니터 캡처 비용까지 없어지는 것은 아니다. zero-copy 재설계는 측정 후 판단한다.

## 후보 B: 같은 HWND의 네이티브 합성

[CreateHostBackdropBrush](https://learn.microsoft.com/en-us/uwp/api/windows.ui.composition.compositor.createhostbackdropbrush)는 창이 그려지기 전 뒤쪽을 compositor에서 참조하는 경로다. 앱이 픽셀을 다시 읽을 수 없고 사용자 투명도·전원 정책의 영향을 받는다. 이 구조는 자체 스크린샷을 반복할 필요를 없애지만 실제 화면 공유 도구에서의 재질 표현은 별도 검증해야 한다.

저장소 이력상 `b7b50be54f12fdb3af478b39be5ac08c97b1585c`에서 `native_material.rs`를 제거하고 GDI 방식으로 복귀했다. 이전 구현은 같은 HWND의 DesktopWindowTarget, HostBackdropBrush, Gaussian blur, 표면별 geometric clip을 사용했다. 기존 형태·클립 코드는 참고할 수 있지만 `DESIGN.md`에 비활성 창의 불투명 회색 실패가 기록되어 있으므로 그대로 되돌리면 안 된다.

[SystemBackdropConfiguration.IsInputActive](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.composition.systembackdrops.systembackdropconfiguration.isinputactive)는 재질이 창을 활성 상태로 취급할지 설정하는 공식 경로다. OS 키보드 포커스를 실제로 가져오는 것과 구분된다. 다만 Microsoft.UI의 backdrop controller 설정이며 기존 Windows.UI.Composition HostBackdropBrush에 직접 붙이는 수정은 아니다. 이 속성 하나로 과거 실패가 해결된다고 주장하지 않는다. Windows App SDK가 필요해지면 의존성과 배포 비용을 별도 검토한다.

검증 순서는 비활성 창에서 움직이는 배경 표시 → WebView 뒤 적층 → 오목한 노치·상세 클립 → 불투명도 0/50/100 → 입력 통과와 포커스 → 외부 캡처다. 고대비·투명도 해제·절전 정책에 따른 단색 대체 표시도 정상 지원 상태로 정의한다.

로컬 Wry 0.55.1/0.57.0 소스는 일반 WebView2 controller 생성 경로를 사용한다. 전체 WebView를 CompositionController로 바꾸는 작업은 단순 옵션 변경으로 가정하지 않는다. 기존 같은 HWND 방식부터 확인하고, WebView 호스팅을 교체해야만 한다면 입력 전달·접근성·드래그까지 포함하는 별도 구조 변경으로 평가한다.

## 함께 바뀌어야 하는 코드와 문서

| 참조점 | 검토할 변경 |
| --- | --- |
| `src-tauri/src/infrastructure/liquid_glass.rs` | 전역 제외·lease 의존 제거. 기존 begin/sync 명령은 현재 사실상 no-op이므로 인터페이스 존재만으로 네이티브 구현이 있다고 판단하지 않기 |
| `src-tauri/src/infrastructure/liquid_capture.rs` | WGC 채택 시 프레임 공급원·영역 읽기·취소·장치 복구 |
| `src-tauri/src/infrastructure/liquid_events.rs` | 새 엔진의 프레임 이벤트와 기존 창 이벤트 중복 필요성 재평가 |
| `src/ui/liquid-glass.ts` | 현재 픽셀 기반 렌더·대비 계산·drag 중 캡처 정지 계약 유지 또는 경로별 명시적 대체 |
| `src/ui/liquid-loop.ts` | 폴링 공급원 교체 시 불필요한 스케줄 제거. 오래된 응답 적용 방지 유지 |
| `src/ui/liquid-contrast.ts` | 네이티브 brush는 픽셀을 읽을 수 없으므로 대비 전략 재설계. 대비 때문에 전역 제외를 재도입하지 않기 |
| `scripts/liquid-events-native-smoke.py` | 기존 OS 캡처 회귀 검사를 유지하고 동시 독립 캡처 검사 추가 |
| `DESIGN.md`, `docs/ARCHITECTURE.md` | 전자는 lease 750ms 설명, 후자는 이미 제거된 native_material 설명이 남아 있음. 채택된 실제 구현에 맞춰 수정 |

네이티브 재질의 대비는 먼저 기존 가독성 하한·wash·그림자만으로 충분한지 검증한다. 창 바깥 픽셀 표본은 실제 내부 배경을 대표한다는 보장이 없어 보조 힌트로만 사용할 수 있다.

## 완료 판정

1. 캡처를 시작·종료·실패·복구하는 모든 경로에서 표시 HWND가 `WDA_NONE`을 유지한다. 주기적 확인만으로 짧은 변경을 놓칠 수 있으므로 호출 경로 검사도 병행한다.
2. 동적 배경·창 이동·노치 드래그·상세 펼침 중 독립 OS 캡처와 60fps 녹화에서 전경 누락이 없다. 최소 60초·3,600프레임을 제안하되 이는 테스트 기준이지 모든 도구에 대한 수학적 보장이 아니다. 고정 전경 마커로 판정하고 캡처 시점도 바꾼다.
3. 전체 모니터 공유, 노치 창 단독 공유, 다른 앱 창만 공유를 구분한다. 다른 앱만 공유할 때 노치가 안 나오는 것은 정상일 수 있다. 창 단독 공유의 재질은 모니터 합성과 동일하다고 가정하지 않되 아이콘·문자 누락은 허용하지 않는다.
4. 자기 화면이 반복 누적되는 잔상·재귀 무늬가 없고 비활성 상태에서 포커스를 빼앗지 않는다.
5. 두 모니터·혼합 DPI·HDR 여부·투명도 정책·잠금/절전 복귀·장치 손실을 확인한다. 미지원 상태는 명시된 캡처 없는 재질로 돌아간다.
6. CPU뿐 아니라 WebView/DWM/GPU를 포함한 비용과 표시 지연을 기존 방식과 비교한다. WGC 또는 compositor라는 이름만으로 성능 개선을 주장하지 않는다.

최초 검토에서는 이 문서만 추가했다. 이후 아래 독립 실험을 수행했다. 제품 코드의 동작·성능 검증과 구분한다.

## 2026-11-01 세션별 제외 실험 결과

**첫 단계 통과:** 현재 Windows 10.0.26200에서 실제 모니터 WGC 세션의 `QueryInterface`가 `S_OK`를 반환했다. `SetWindowExclusionList`를 적용한 세션에는 가려진 배경이 보이고 적용하지 않은 별도 세션에는 전경 창이 남았다. 전역 `SetWindowDisplayAffinity`는 호출하지 않는다.

MSVC와 설치된 Windows SDK만 사용하는 `scripts/liquid-wgc-capability.cpp` 및 `.ps1`을 추가했다. 제품 의존성·설정은 변경하지 않았다. SDK에 없는 인터페이스 ABI는 [Microsoft windows-rs의 고정 커밋](https://github.com/microsoft/windows-rs/blob/317646c941aa18916ffa5daeb4ffd5ffa4cd26f3/crates/libs/windows/src/Windows/Graphics/Capture/mod.rs)에서 확인했다. 참조 파일 SHA-256은 `7A0D2865B5CCCC5E865CCF6603C30D9B66696AC4ECC4F62BCF2209B36961C984`이다. `IInspectable` 기반이며 메서드 순서는 Set, Get이다. 추정 vtable을 호출한 것이 아니다.

실험은 비활성 topmost 창 두 개를 만든다. 뒤쪽 창은 빨강/파랑으로 바뀌고 앞쪽 창은 자홍색이다. 같은 모니터를 두 WGC 세션에서 동시에 읽되 첫 세션에서만 앞쪽 창을 제외한다. 각 프레임의 가림 영역 한 픽셀과 GDI 화면 한 픽셀을 비교한다. 6초 관측 중 초기 0.5초는 준비 구간으로 제외한다.

| 측정 | 최초 실행 | 재실행 | 제외를 끈 대조 실험 |
| --- | --- | --- | --- |
| 자체 WGC: 배경 확인 | 125/125 | 267/267 | 0/265 |
| 자체 WGC: 빨강 / 파랑 | 62 / 63 | 133 / 134 | 0 / 0 |
| 별도 WGC: 전경 유지 | 125/125 | 268/268 | 267/267 |
| GDI: 전경 유지 | 55/55 | 55/55 | 55/55 |
| `WDA_NONE` 이탈 관측 | 0 | 0 | 0 |

대조 실험은 자체 캡처의 265프레임 모두가 배경 판정에 실패해 의도대로 exit 1을 반환했다. 별도 WGC와 GDI는 정상이다. 단순히 실행 오류를 대조 실험 성공으로 간주하지 않았다.

```powershell
# 창 표시나 캡처 시작 없이 실제 세션의 인터페이스만 조회
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/liquid-wgc-capability.ps1
# 약 6초간 작은 색상 창 두 개를 표시하고 동시 캡처
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/liquid-wgc-capability.ps1 -CaptureTest
# 위 명령으로 빌드 후 실행. 배경 판정 실패와 exit 1이 기대 결과
& .\tmp\liquid-wgc-capability\probe.exe --control
```

결과는 `tmp/liquid-wgc-capability/`에 저장된다. 별도 코드 검토에서도 이 짧은 가능성 실험을 막는 결함은 발견되지 않았다. `npm test`와 `npm run build`도 통과했다.

**해석의 한계:** 별도 WGC는 같은 프로세스의 독립 세션이며 외부 녹화 앱 자체가 아니다. 일반 불투명 테스트 창을 사용했으므로 투명 Tauri/WebView2 노치, 전체 표면, HDR·혼합 DPI, 장시간 실행, 최신 배경 반영 지연과 성능은 아직 검증하지 않았다. 고정된 주 모니터 좌표와 정확한 RGB 비교를 쓰므로 다른 환경에서는 추가 조정과 검증이 필요하다.

**다음 단계 판단:** 세션별 제외 경로를 우선 후보로 유지한다. 실제 노치 HWND를 대상으로 하는 격리 preview에 연결하고, 기존 GDI 캡처·전역 affinity 변경을 끈 상태에서 같은 검사를 반복한다. 미지원 환경에는 캡처 없는 재질을 제공하는 정책과 프레임 수명·모니터 변경·장치 복구를 제품 통합 전에 확정한다. 이번 결과만으로 제품의 화면 공유 문제가 해결됐다고 선언하지 않는다.

## 실제 노치에 연결한 debug preview 결과

`TOKENUSAGE_WGC_PREVIEW=1`을 설정한 debug 빌드에 한해 `liquid_wgc.rs`를 사용하도록 연결했다. 기본/릴리스 빌드는 기존 GDI 경로를 유지한다. 기존 LQ01/BMP 프레임 형식과 Canvas 유리 효과를 유지하며, WGC 실패 시 GDI로 전환하지 않는다. `windows-collections 0.2`는 이미 의존성 트리에 있던 Microsoft 패키지를 직접 참조한 것이다.

두 번의 실제 노치 검사에서 동적 배경 중 OS 스크린샷 **각 100/100장**에 노치가 포함됐다. 각 실행의 affinity 표본 **100/100개가 0**이었다. 첫 실행 배경 변경→Canvas 반영 p50/p95는 31/58ms, 최종 실행은 31/32ms였다(각 20표본). 첫 이벤트만 보면 각각 68ms, 53ms였다. 이는 화면 표시 지연이나 모든 녹화 앱에 대한 보장값이 아니다.

추가 확인:

- 두 실제 모니터 × 네 방향, 총 8개 배치와 설정 패널의 공통 높이·안정된 HWND 유지.
- 연속 재로드 10회와 약 15.6초 관찰에서 오류 없음. 장시간 누수 검증으로 해석하지 않는다.
- Dark/Glass/Liquid 실제 WebView 검사 통과. 각 테마 설정 탭 높이 440 DIP, 45프레임 표본에서 창·노치 위치 유지.
- 드래그 상태를 적용하자 세션 종료 로그가 발생했고, 600ms 유지 중 재시작이 없었다. 해제 후 세션이 재생성되고 렌더러가 준비 상태로 돌아왔다. 실제 손 드래그 전체 이동 경로의 성능 측정은 아니다.
- 원본 OS 이미지 `tmp/liquid-events-native.png`를 직접 확인해 유리 표면·아이콘이 포함된 것을 확인했다.

실행 중 발견해 수정한 문제:

1. windows-core 0.61은 `TryGetNextFrame`의 정상적인 S_OK/null을 `Error::empty()`로 반환한다. 이 경우만 프레임 대기로 처리하며 실제 HRESULT 실패는 전파한다.
2. 재로드 시 worker 캐시를 비운다. 빠른 초기화 요청으로 큐가 찬 경우에도 epoch를 증가시켜 초기화가 유실되지 않도록 했고, 이전 epoch의 대기 캡처는 처리하지 않는다.
3. 렌더러가 중단될 때 단순 lease 해제가 아니라 native 자원을 해제한다. 따라서 WGC는 드래그/숨김 중에도 계속 캡처하는 세션을 유지하지 않는다. 기존 GDI worker의 무기한 idle 대기는 유지하며 WGC의 만료 점검만 시간 제한 대기를 사용한다.

### 새로 확인된 배포 제약: OS 캡처 테두리

**현재 preview에는 노란 WGC 캡처 테두리가 보인다.** [Microsoft IsBorderRequired 문서](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired)에 따르면 테두리 제거는 `GraphicsCaptureAccess.RequestAccessAsync(Borderless)`를 통한 사용자 동의와 capability 처리가 필요하다. 단순히 false를 설정해 성공했다고 해서 테두리가 실제로 사라졌다는 뜻은 아니다. 이 preview는 권한 프롬프트를 띄우거나 정책을 우회하지 않는다.

따라서 현재 결과는 **실제 노치의 누락 문제를 피할 수 있다는 증거**이며, 곧바로 기본 배포할 수 있다는 증거는 아니다. 다음 배포 판단 전에는 현재 NSIS/MSI·비패키지 실행 환경의 공식 borderless 동의 경로, 미지원 OS의 재질, 독립 녹화 앱, HDR/혼합 DPI, 절전·장치 손실 및 GPU 비용을 검증해야 한다. WGC는 정적 1Hz 요청 중에도 모니터 세션을 유지하므로 기존 rAF 감소 수치를 에너지 절감으로 승계하지 않는다.

원시 결과: `tmp/liquid-wgc-capability/notch-smoke-final.log`, `environment.log`, `quality.log`, `drag-lifecycle.json`. 제품 기본 백엔드는 전환하지 않았고 배포하지 않았다.

### 후속 실험: 공식 borderless 권한 경로

`TOKENUSAGE_WGC_BORDERLESS_PREVIEW=1`을 추가하면 WGC worker가 프로세스당 한 번 `GraphicsCaptureAccess.RequestAccessAsync(Borderless)`를 호출한다. `Allowed`일 때만 `SetIsBorderRequired(false)`를 적용한다. 거부·오류는 실패로 반환하고 OS 설정을 바꾸거나 GDI로 되돌아가지 않는다. 권한 결정 대기는 capture worker에서 일어나며, 새로운 환경에서는 실제 Windows 동의가 필요할 수 있다.

현재 비패키지 debug preview에서는 OS가 **Allowed(4)**를 반환했다. 이후 OS 캡처 이미지를 직접 확인하여 노란 테두리가 사라지고 유리 효과·아이콘은 유지되는 것을 확인했다. API setter의 성공만으로 테두리 제거를 판정한 것이 아니다.

- 테두리 제거 후 동적 배경 스크린샷: **100/100장 노치 포함**, affinity **100/100개가 0**.
- 배경 변경→Canvas 반영 20표본: p50 **30ms**, p95 **36ms**; 첫 이벤트 **56ms**.
- 비교 이미지: `tmp/liquid-wgc-capability/notch-before-borderless.png`, `notch-after-borderless.png`.
- 결과: `tmp/liquid-wgc-capability/borderless-smoke.log`.

이 결과는 현재 PC의 허용 상태를 증명한다. 모든 사용자·기업 정책·설치 방식에서 자동 허용된다는 뜻은 아니다. 실제 권한 거부 시의 사용자 안내, 정식 배포본, 다른 녹화 앱과 동시 실행 및 성능 비교는 다음 제품화 검증에 남는다.

공통 native-quality 검사도 기본 GDI와 WGC 모두 세 테마에서 통과했다. 중간에 순간적인 두 번의 커서 이동에서 상세가 열리지 않는 검사가 실패했고, 실제 포인터 진입 이벤트로 정상 동작을 확인한 뒤 두 이동을 각각 150ms의 연속 이동으로 바꿨다. 45프레임 상세 표시 유지 검증은 그대로 유지했다. 해당 결과는 `default-quality-final.log`, `wgc-quality-final.log`에 있다.
