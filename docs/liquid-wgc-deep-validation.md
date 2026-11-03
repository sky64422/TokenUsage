# WGC 추가 검증 — 2026-11-01

> Release status (2026-11-03): v0.5.18 promotes WGC and official borderless access to the default. Preview references below describe the historical test setup; unsupported/denied capture uses the base material without GDI fallback.

## v0.5.18 release gate (2026-11-03)

- `npm test`: 291 passed, one live-provider test ignored; build and Clippy passed.
- Browser UI: 9/9; Liquid drag geometry: 72 samples; scheduling: idle 3 reads/3s, drag 90 reads/1.5s.
- Signed release executable with both debug overrides set to `0`: WGC ready, 100/100 OS screenshots retained the notch, independent WGC observer 200/200 retained it, display affinity 0 throughout.
- Background-event latency: 20 samples, p50 29ms, p95 30ms, maximum 67ms. This measures background change response, not GPU presentation latency.
- Failure fallback is covered by the renderer unit test. Native denial injection could not replace Tauri's read-only IPC method; actual denied-policy hardware remains unverified.
- Native smoke uses Win32 absolute positioning for its background fixture, because Tk negative coordinates mean right/bottom anchoring rather than negative desktop coordinates.

대상은 `TOKENUSAGE_WGC_PREVIEW=1`과 `TOKENUSAGE_WGC_BORDERLESS_PREVIEW=1`로 실행한 격리 debug preview다. 기본 배포 백엔드는 여전히 GDI다. [설계·첫 실험 기록](liquid-screen-sharing-review.md)에서 이어지는 검증이며, 실제 화상회의 서버로 화면을 전송하지 않았다.

## 별도 프로세스 캡처

`liquid-wgc-capability.cpp --observe`는 앱과 **별도 프로세스**에서 모니터 WGC 세션을 만든다. 제외 목록을 설정하지 않으며, 변하는 단색 배경 위 노치 영역의 내부 색상 다양성을 검사한다. 캡처 표시 테두리를 판정에 이용하지 않도록 ROI 외곽 3픽셀은 제외한다.

| 검사 | 결과 |
| --- | --- |
| 동적 배경 OS 스크린샷 | 300/300장 노치 포함 |
| 전역 affinity 표본 | 300/300개가 0 |
| 별도 프로세스 WGC | 관측 200프레임 모두 노치 포함 |
| 노치 렌더링을 숨긴 대조 실험 | 관측 200프레임 모두 누락으로 감지 |

별도 프로세스 결과는 같은 프로세스 내 세션 실험보다 범위가 넓다. 다만 이 검사는 단색 테스트 배경에서 **노치 전체의 존재/누락**을 판정한다. 모든 아이콘·문자의 완전성, 임의 배경, Teams/Zoom/OBS의 개별 캡처 경로를 증명하지 않는다. 관측 200프레임은 고정 FPS 녹화나 연속된 모든 디스플레이 프레임이라는 뜻이 아니다. 프레임 풀·폴링에서 중간 프레임은 건너뛸 수 있다.

증거: `tmp/liquid-wgc-capability/independent-observer.log`, `independent-observer-control.log`.

## 실패·복구 및 발견한 수정 사항

격리 preview HWND를 실제 가상 데스크톱 밖으로 잠시 이동시켜 WGC가 유효한 모니터를 찾을 수 없게 했다. 오류는 `WGC preview (no GDI fallback): WGC region has no monitor`로 반환됐고, affinity는 0이었다. 창을 원래 위치로 복구하고 reload하자 렌더러가 다시 준비 상태가 됐다. 테스트는 예외 발생 시에도 위치를 복구한다.

초기화 명령 20개를 동시에 보낸 뒤 새 캡처를 요청하는 과정에서 **실제 큐 포화 오류**를 발견했다. 기존 `try_send`의 Full 오류는 프런트엔드에서 캡처 실패로 취급되어 루프를 중단시킬 수 있었다.

- 재현 단위 테스트에서 `Err("Liquid capture queue: sending on a full channel")`를 확인했다.
- 포화는 기존의 빈 프레임 응답으로 반환하여 다음 주기에 다시 읽게 했다. 큐 크기를 늘리거나 메인 스레드를 막지 않았다.
- worker 연결 끊김은 여전히 오류로 반환한다. 이 구분도 같은 테스트로 검증했다.
- 수정 후 실제 native 검사에서 20개 초기화 요청을 처리한 뒤 새 LQ01 프레임 268바이트를 받았다.

이는 잘못된 소스 위치와 요청 혼잡에 대한 검증이다. 실제 디스플레이 장치 손실, GPU reset, 절전 복귀, 모니터 케이블 분리와는 구분한다. 실제 OS 권한 거부·미지원 Windows에서의 실행도 이번에는 검증하지 않았다.

증거: `congestion-red.log`, `congestion-green.log`, `recovery-final.log`, `recovery-result.json`(모두 `tmp/liquid-wgc-capability/`).

## CPU·메모리 비교

같은 현재 렌더러에 GDI→WGC→WGC→GDI 순서로 preview를 재시작했다. 진단 파일 로그는 끄고 각 부하를 약 8초씩 관측했다. 다음 수치는 각 백엔드 2회 평균이며 **앱 + WebView2 자식 프로세스 7개의 합계**다. CPU는 논리 CPU 12개 전체에 대한 백분율이다.

| 부하 | GDI CPU | WGC CPU | GDI private memory | WGC private memory |
| --- | ---: | ---: | ---: | ---: |
| 정적 배경 | 0.268% | 0.337% | 235.6 MiB | 290.5 MiB |
| 뒤쪽 창 이동·색상 변경 | 3.946% | 3.184% | 238.1 MiB | 282.1 MiB |
| 위치 고정·색상 변경 | 2.965% | 2.361% | 236.1 MiB | 290.9 MiB |

창 이동 조건의 변경 paint 수는 GDI 109/108회, WGC 110/110회로 비슷했다. 위치 고정 색상 변경은 GDI 99/85회, WGC 86/88회여서 처리량이 완전히 동일하지 않다. 짧은 desktop 측정이며 반복 간 편차가 있어 CPU 절감률을 보장하지 않는다. 정적 CPU는 오히려 조금 높았고, private memory는 WGC가 약 44~55MiB 더 사용했다. GPU 전용 메모리는 이 수치에 포함된다고 가정하지 않는다.

DWM CPU는 현재 `Get-Process`에서 값이 제공되지 않았다. 최초 계측 코드가 빈 값을 합산해 0으로 만들던 문제를 발견해 **null(미측정)**로 수정하고 비교를 다시 실행했다. 최종 수치는 `tmp/liquid-backend-{gdi-1,wgc-2,wgc-3,gdi-4}.json`이다. 초기 `backend-comparison-initial.log`의 DWM 0은 유효한 측정값으로 사용하지 않는다.

검사 코드에는 preview의 실행 파일 확인과 launcher PID의 자손 여부 검증을 추가했다. 설치 앱과 제목이 같아도 잘못된 HWND/프로세스 트리를 측정하는 것을 허용하지 않는다.

## 3분 자원 관찰

WGC에서 두 실제 모니터 × 네 방향 8개 배치와 reload 10회를 검사한 뒤 180.84초 관찰했다. 두 모니터는 100% 배율이며 2560×1440, 1024×1280이다.

| 항목 | 관찰 |
| --- | --- |
| 설정 탭 공통 높이 | 큰 모니터 440 DIP, 작은 모니터 421 DIP |
| 렌더러 상태 | 관측 내내 ready |
| Canvas / DOM | 2 / 304 유지 |
| JS heap | 약 9.04~11.54 MiB |
| 전체 private memory | 612.62 → 353.36 MiB |
| 핸들 합계 | 3,821 → 3,767 |
| 프로세스 수 | 7 유지 |
| 페이지 오류 | 0 |

초기 수치는 설정·배치 변경·reload 직후라서 안정 상태의 CPU 비교 RAM보다 높다. 이 짧은 관찰에서 지속 증가를 발견하지 않았다는 의미이며, 장시간 누수가 없다는 증명은 아니다.

증거: `tmp/liquid-wgc-capability/wgc-environment-results.json`, `wgc-environment-180.log`.

## GPU 측정 방법

기존 단발 `Win32_PerfFormattedData`의 0만으로 GPU 사용량을 판단하지 않는다. `liquid-gpu-sample.ps1`은 raw GPU engine의 `UtilizationPercentage`가 `PERF_100NSEC_TIMER(542180608)`인 것을 확인하고, 약 5초 떨어진 두 표본의 값과 `Timestamp_Sys100NS` 차이로 계산한다. [Microsoft의 계산식](https://learn.microsoft.com/en-gb/windows/win32/perfctrs/calculating-counter-values)을 따른다.

동일한 엔진 이름으로 매칭된 행만 계산한다. 앱 프로세스 트리와 DWM을 나누며 **여러 엔진 백분율의 합을 전체 GPU 사용률로 해석하지 않는다.** DWM에는 다른 창의 합성도 포함된다. CPU ABBA와 별도의 GDI/WGC 실행으로 정적·창 이동·색상 변경·노치 밖 색상 변경을 관측한다.

각 조건 1회, 약 5.06~5.09초의 raw-counter 구간이다. GDI는 41행, WGC는 55행이 시작·종료 양쪽에서 매칭됐다. 다음 값은 해당 프로세스들의 **GPU 엔진 활동 백분율 합**이며 전체 GPU 사용률이 아니다.

| 부하 | GDI 앱 엔진 합 | WGC 앱 엔진 합 | GDI DWM 엔진 합 | WGC DWM 엔진 합 |
| --- | ---: | ---: | ---: | ---: |
| 정적 배경 | 0.002 | 0.013 | 0.978 | 0.729 |
| 뒤쪽 창 이동·색상 변경 | 0.116 | 0.401 | 2.123 | 1.679 |
| 위치 고정·색상 변경 | 0.103 | 0.380 | 1.932 | 1.653 |
| 노치 밖 색상 변경 | 0.000 | 0.027 | 1.103 | 0.991 |

WGC 앱 엔진 활동은 높게 관측됐다. 노치 밖의 변화에서는 두 백엔드 모두 Canvas paint 0회였지만 WGC 엔진에는 활동이 있었다. 모니터 전체 세션·프레임 획득에 추가 비용이 있을 수 있다는 근거다. DWM 수치는 반대 방향으로 움직였으나 다른 창·시스템 합성 부하가 섞이므로 그 차이를 전부 TokenUsage의 절감으로 귀속하지 않는다. 각 조건 1개 구간이므로 이 표만으로 전체 GPU 또는 에너지의 우열을 판정하지 않는다.

증거: `tmp/liquid-backend-gdi-gpu-1.json`, `tmp/liquid-backend-wgc-gpu-2.json`. 이 후속 실행에서는 benchmark HWND의 실행 파일과 launcher PID 자손 여부 검증도 통과했다.

## 판단

- 세션별 제외는 실제 노치와 별도 캡처 프로세스가 동시에 동작할 때의 누락 문제를 피하는 유효한 방향이다.
- 큐 혼잡으로 루프가 중단될 수 있는 문제를 발견해 수정했고, 실패→복구와 초기화 폭주 후 새 프레임을 native 환경에서 확인했다.
- 그러나 메모리가 추가로 필요하고 앱 GPU 엔진 활동도 늘었다. **더 가볍다는 목표는 아직 달성됐다고 보지 않는다.** 전체 모니터 프레임 보유·복사 비용과 불필요한 영역 변경 처리부터 최적화할 근거가 생겼다.
- 기본 배포 전환은 보류한다. 실제 OS 권한 거부/미지원 Windows의 재질·안내, 실제 녹화 앱, 장치 손실·절전·혼합 DPI/HDR은 별도 검증이 필요하다. 이번 테스트가 그 검증을 대신하지 않는다.

## 재현

Vite 1524가 실행 중이어야 한다. 비교 드라이버 실행 전 CDP 9223 preview는 닫는다. 드라이버는 자신이 띄운 프로세스 트리만 종료한다.

```powershell
python -X utf8 scripts/liquid-backend-comparison.py
python -X utf8 scripts/liquid-backend-comparison.py --raw-gpu
# 아래 검사는 WGC + borderless 격리 preview를 별도로 띄운 후 순차 실행
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/liquid-wgc-capability.ps1
python -X utf8 scripts/liquid-events-native-smoke.py --screenshots --screen-samples 300 --latency-samples 20 --require-visible --observer
python -X utf8 scripts/liquid-events-native-smoke.py --screen-samples 300 --observer --observer-control
python -X utf8 scripts/liquid-wgc-recovery-smoke.py
python -X utf8 scripts/liquid-environment-smoke.py --seconds 180
```

## 창 영역 복사 최적화 (2026-11-01 후속)

`liquid_wgc.rs`는 추가로 만들던 전체 모니터 텍스처 대신 **고정 HWND 캔버스와 모니터의 교집합**만 복사한다. 닫힌 상세 패널이 앞으로 사용할 영역도 포함하므로, 정지 화면에서 새 위치·크기·generation의 첫 요청을 처리할 수 있다. 원본 WGC 프레임은 복사 성공·실패 모두 즉시 Close하고, rail/detail staging 텍스처는 슬롯별로 재사용한다. 모니터 크기 WGC frame pool 자체는 남아 있다. 전체 화면 캡처를 ROI 캡처 API로 바꾼 것은 아니다.

처음 시도한 원본 프레임 하나의 지속 보유는 철회했다. pool 2개 중 1개를 점유한 안의 응답 지연은 p50/p95 **65/73ms**, 즉시 반환하는 전체 화면 복사 대조군은 **31/37ms**였다. Microsoft의 [프레임 처리 설명](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture#process-capture-frames)도 프레임의 신속한 반환과 반환 후 surface 참조 금지를 설명한다. 최종 코드는 별도 텍스처에 창 영역을 복사하고 프레임을 반환한다.

동일 renderer·격리 preview·debug logging OFF에서 수정 전과 최종 안을 각각 한 번 실행했다. 각 CPU 구간은 약 8초, raw GPU 구간은 약 5초다. CPU는 앱+WebView 프로세스의 전체 머신 기준이고, GPU는 앱 엔진 백분율 합이다.

| 부하 | CPU 전→후 (%) | Private RAM 전→후 (MiB) | 앱 GPU 엔진 합 전→후 | Paint 전→후 |
| --- | ---: | ---: | ---: | ---: |
| 정적 배경 | 0.233→0.230 | 283.77→275.26 | 0.0301→0.0133 | 0→0 |
| 창 이동·색 변경 | 3.078→2.926 | 280.61→270.05 | 0.4699→0.4175 | 111→112 |
| 위치 고정·색 변경 | 1.939→1.486 | 289.89→278.56 | 0.2548→0.1806 | 83→73 |
| 노치 밖 색 변경 | 0.202→0.278 | 286.75→272.35 | 0.0242→0.0094 | 0→0 |

RAM은 약 **8.5~14.4MiB 감소**했다. 창 이동 paint 수는 유사하지만 고정 위치 색 변경은 다르므로 그 CPU 감소를 동일 작업량의 효율 향상으로 단정하지 않는다. 노치 밖 CPU는 소폭 높았고 DWM 엔진 합은 조건별로 양방향 변했다. 1회 표본이며 전체 GPU·전력 절감을 입증하지 않는다.

증거: `tmp/liquid-backend-wgc-roi-before.json`, `tmp/liquid-backend-wgc-roi-final.json`. 철회한 보유 안은 `tmp/liquid-backend-wgc-roi-after.json`, 지연 대조는 `tmp/liquid-roi-events-after.json`와 `tmp/liquid-roi-events-control.json`이다.

최종 코드 검증:

- 서로 다른 위치·크기·generation의 정적 두 색 ROI **40/40** 실제 RGBA 일치. 모니터 밖 실패·복구, reset 20회 후 새 LQ01 프레임 통과.
- OS 스크린샷 **300/300**, 별도 WGC 프로세스 **200/200** 노치 표시. affinity **300/300 = 0**, page error 없음. 저장된 native 이미지도 육안 확인.
- 정지 후 첫 변경 68ms, 20개 표본 p50 **31ms**, p95 **63ms**. 중앙값은 대조군 수준으로 복원됐으나 tail 지연 개선을 입증하지 않는다.
- dark/glass/liquid 모두 settings 3탭 **440/440/440 DIP**, hover 45프레임과 창·rail 위치 유지 통과.
- 실제 모니터 2개 × 4방향 **8개 배치**, reload **10회**, 이후 **60.7초** ready 관찰 통과. 장시간 누수·혼합 DPI·HDR·절전/장치 손실 보장은 아님.
- `npm test`: UI 109, Rust library 128(+1 ignored), risk 19, liquid frame 3, notch geometry 28 통과. `npm run build`, Python compile, `git diff --check` 통과.

최종 증거는 `tmp/liquid-roi-final-{events.json,recovery.log,quality.log,environment.json,tests.log,build.log}`에 있다. 기본 GDI 경로 및 WGC debug opt-in 정책은 유지한다. 이번 변경으로 전체 모니터 추가 복사와 교차 surface 버퍼 재할당은 줄였지만, 모든 환경에서 더 가볍거나 화면 공유가 완성됐다는 결론은 아니다.
