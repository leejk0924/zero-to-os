# 구조

지금 코드의 구조를 적는다. 단계가 끝날 때마다 실제 코드에 맞게 고친다. 앞으로 만들 것은 `docs/spec.md`의 로드맵과 단계별 설계에 있다.

## 현재 상태

1단계까지의 코드다(이슈 #1, PR #10·#11·#12·#14). 코어 4개가 M 모드에서 시작해 각자 S 모드로 내려가 `[S] hart N 시작`을 출력하고 `wfi`로 멈춘다. 트랩 처리, 타이머, 메모리 할당, 페이징, 입력은 아직 없다. 외부 크레이트는 쓰지 않는다.

## 부팅 흐름

```
QEMU 리셋 벡터(0x1000) — 모든 코어가 동시에
  → 0x80000000 `_start`        src/entry.S, M 모드
       a0 = mhartid. 4 이상이면 park(wfi 루프)
       sp = __stack_start + (hartid + 1) × 64KiB
       0번 코어: .bss를 8바이트씩 0으로 채움
       나머지:   BOOT_READY가 0이 아닐 때까지 대기 → fence r, rw
  → `start(hartid)`            src/main.rs, M 모드
       0번 코어: uart::init() → BOOT_READY = 1 (Release) → "[M] hart 0: mstatus=..." 출력
       모든 코어: mstatus.MPP = S, mepc = kmain, satp = 0,
                  medeleg/mideleg = 0xffff, PMP 항목 0으로 전체 메모리 허용,
                  tp = hartid → mret
  → `kmain()`                  src/main.rs, S 모드
       "[S] hart N 시작" 출력 → wfi 루프
```

- 0번 코어는 `BOOT_READY`를 세운 **뒤에** 출력하므로, 다른 코어의 `[S]` 줄이 `[M]` 줄보다 먼저 나올 수 있다.
- `stvec`을 설정하지 않았다. S 모드에서 예외가 나면 주소 0으로 뛰어 `fault_fetch`를 끝없이 반복한다(2단계에서 해결).

## 메모리 배치

`kernel.ld`가 정한다. 페이징이 꺼져 있어 모든 주소가 물리 주소다.

| 영역 | 시작 | 내용 |
|---|---|---|
| UART (MMIO) | `0x1000_0000` | NS16550A 레지스터 |
| `.text` | `0x8000_0000` | 맨 앞에 `.text.entry`(`_start`) 고정, 그다음 나머지 코드 |
| `.rodata` | `.text` 뒤 | 문자열 상수 등 |
| `.data` | `.rodata` 뒤 | `.sdata` 포함. `BOOT_READY`가 여기 있다 |
| `.bss` | `__bss_start` ~ `__bss_end` | `.sbss` 포함, 앞뒤 8바이트 정렬. 0번 코어가 지운다 |
| 부트 스택 | `__stack_start` (16바이트 정렬) | `NOLOAD`, 코어당 64KiB × 4. 코어 N의 `sp` 초깃값은 N+1번째 64KiB의 끝 |

디버그 빌드 기준 `.text` 약 8KiB, 부트 스택 256KiB. QEMU 메모리는 128MiB(`-m 128M`)이고 커널 뒤의 나머지는 아직 쓰지 않는다.

## 모듈

| 파일 | 하는 일 | 밖에서 쓰는 것 |
|---|---|---|
| `src/entry.S` | `_start`: 코어 번호 확인, 스택, `.bss` 지우기, 부팅 대기, `call start` | 심볼 `_start` |
| `src/main.rs` | `start()`(M 모드 설정, `mret`), `kmain()`(S 모드 진입점), `hartid()`, 패닉 처리기, `BOOT_READY` | `hartid()` — `tp`에서 코어 번호를 읽는다 |
| `src/uart.rs` | NS16550A 초기화와 출력. 출력은 `static UART: SpinLock<Uart>`를 거친다 | `init()`, `putc()`, `print!`/`println!`, `print_panic()` |
| `src/spinlock.rs` | `SpinLock<T>`: `AtomicBool` + `compare_exchange_weak`(Acquire) / `store(false, Release)`. 인터럽트는 끄지 않는다 | `new()`, `lock()` → 가드, `try_lock()` |
| `kernel.ld` | 위의 메모리 배치 | `__bss_start`, `__bss_end`, `__stack_start` |
| `build.rs` | 링커 스크립트 지정, `kernel.ld`가 바뀌면 다시 링크 | |
| `.cargo/config.toml` | 기본 타깃 `riscv64gc-unknown-none-elf`, `cargo run` = QEMU 실행 | |
| `scripts/check.sh` | 빌드 후 QEMU를 2초 띄워 시리얼 출력에 기대 문자열이 모두 있는지 검사. `--release`, `SMP=<코어 수>` | |

### 출력 경로

- `print!`/`println!` → `uart::_print` → `UART.lock()` → `write_fmt` → `putc`. 서식 처리부터 줄바꿈까지 락 하나 안에서 나가므로 코어끼리 줄이 섞이지 않는다.
- 패닉 → `PANICKING.swap(true)`로 첫 코어만 진행 → `uart::print_panic`: UART 락을 `try_lock`으로 최대 100,000번 시도해 얻으면 출력하고 락을 돌려주지 않는다(`mem::forget`). 못 얻으면 락 없이 출력한다. 나머지 코어는 출력 없이 `wfi`.

## 다음 단계가 알아야 할 것

- `tp`는 코어 번호 전용이다. 다른 용도로 쓰지 않는다.
- 스핀락은 인터럽트를 끄지 않는다. 인터럽트를 켜면 락을 쥔 코어가 처리기에서 같은 락을 잡아 스스로 교착될 수 있다.
- 패닉 뒤에는 UART 락이 풀리지 않으므로 다른 코어의 `println!`은 영원히 기다린다(의도).
- 하드웨어와 무관한 로직을 분리한 크레이트는 아직 없다(D17, 2단계부터).
