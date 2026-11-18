# 1단계 구현 계획: 부팅 → 여러 코어 시작 → Rust 커널 진입 → 글자 출력

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**목표:** `cargo run` 한 번으로 QEMU가 뜨고, 코어 4개가 M 모드에서 시작해 각자 S 모드로 내려가 `[S] hart N 시작`을 섞이지 않게 출력한다.

**구조:** QEMU `virt` 머신을 펌웨어 없이 띄우면 모든 코어가 `0x80000000`의 `_start`(어셈블리)에서 동시에 시작한다. 각 코어는 자기 스택을 잡고, 0번 코어만 `.bss`와 UART를 초기화하는 동안 나머지는 `BOOT_READY`를 기다린다. 이후 모든 코어가 Rust `start(hartid)`(M 모드)에서 CSR을 설정하고 `mret`로 `kmain()`(S 모드)에 진입한다. UART는 `SpinLock`으로 보호한다.

**기술:** Rust stable, 타깃 `riscv64gc-unknown-none-elf`, `qemu-system-riscv64` 11.0.2, 외부 크레이트 없음

**설계 문서:** `docs/decision/specs/2026-11-14-boot-hello-design.md`

## 전체 제약

- 작업 흐름, 검토 방식, 학습 노트 규칙은 `AGENTS.md`를 따른다.
- 외부 크레이트를 쓰지 않는다. CSR 접근은 `core::arch::asm!`으로 직접 작성한다.
- 빌드 도구는 `cargo`만 쓴다. Makefile을 두지 않는다.
- QEMU 옵션: `-machine virt -bios none -smp 4 -m 128M -nographic`
- 지원하는 코어는 최대 4개(`MAX_HARTS = 4`). 번호가 4 이상인 코어는 `_start`에서 `wfi` 루프로 멈춘다.
- 코드 주석은 한국어로, "무엇을"보다 "왜 필요한지"를 적는다. 식별자는 영어.
- 이번 단계에서 하지 않는 것: 인터럽트·트랩 처리, 타이머, 메모리 할당기, 페이징, 입력.
- 임시 실험 코드는 확인 후 제거하고 커밋하지 않는다.
- QEMU 종료: `Ctrl-A` 다음 `X`.

## 검토 초점

각 항목은 태스크의 테스트로 고정한다.

1. **대기 신호가 `.bss`에 놓임** — `BOOT_READY`가 `.bss`에 있으면 0번 코어가 `.bss`를 지우는 것과 겹친다. `.data`에 있어야 한다. → 태스크 2의 심볼 주소 확인
2. **코어 수가 4가 아닐 때** — 코어 1개면 한 줄만, 8개면 0~3번 네 줄만 나오고 멈추지 않아야 한다. → 태스크 4의 `SMP=1`, `SMP=8` 확인
3. **릴리스 빌드에서 출력이 사라지거나 락이 깨짐** — `--release`에서도 결과가 같아야 한다. → 태스크 2, 3, 4
4. **패닉 시 아무것도 안 보임** — 락을 쥔 코어가 패닉해도 메시지가 나와야 한다. → 태스크 3
5. **`kernel.ld`를 고쳐도 다시 링크되지 않음** → 태스크 1

## 파일 구조

| 파일 | 역할 | 태스크 |
|---|---|---|
| `rust-toolchain.toml` | stable, 타깃 `riscv64gc-unknown-none-elf`, 컴포넌트 `llvm-tools` | 1 |
| `Cargo.toml` | 패키지 `zero-to-os`, edition 2024, 의존성 없음 | 1 |
| `.cargo/config.toml` | 기본 타깃, `cargo run` 러너(QEMU) | 1 |
| `build.rs` | 링커 스크립트 지정, 변경 시 재빌드 | 1 |
| `kernel.ld` | 메모리 배치 | 1 |
| `src/entry.S` | `_start` | 1, 2 |
| `src/main.rs` | `start()`, `kmain()`, 패닉 핸들러 | 1~4 |
| `src/uart.rs` | UART 출력, `print!`/`println!` | 2, 3 |
| `src/spinlock.rs` | `SpinLock<T>` | 3 |
| `scripts/check.sh` | QEMU를 잠깐 띄워 출력 검사 | 1 |
| `.gitignore` | `/target` | 1 |

---

### Task 1: 빌드 뼈대와 어셈블리에서 글자 출력

**파일:** 생성 `rust-toolchain.toml`, `Cargo.toml`, `.cargo/config.toml`, `build.rs`, `kernel.ld`, `src/entry.S`, `src/main.rs`, `scripts/check.sh`, `.gitignore`

**인터페이스 (만드는 것):**
- 링커 심볼 `__bss_start`, `__bss_end`, `__stack_start` (태스크 2가 사용)
- 섹션 `.text.entry` (`_start`가 들어가며 이미지 맨 앞에 놓임)
- `scripts/check.sh [--release] "<기대 문자열>"...` — 출력에 모든 문자열이 있으면 종료 코드 0. 환경 변수 `SMP`로 코어 수를 바꾼다(기본 4).

- [ ] **1단계: 확인 스크립트 작성** — `scripts/check.sh` (실행 권한 부여)

macOS에는 `timeout` 명령이 없으므로 백그라운드로 띄운 뒤 종료한다.

```sh
#!/bin/sh
# 사용법: scripts/check.sh [--release] "<기대 문자열>"...
# 커널을 빌드해 QEMU로 2초간 실행하고, 시리얼 출력에 기대 문자열이 모두 있는지 검사한다.
set -e
profile=debug
flag=
if [ "$1" = "--release" ]; then profile=release; flag=--release; shift; fi
cargo build $flag
out=$(mktemp)
qemu-system-riscv64 -machine virt -bios none -smp "${SMP:-4}" -m 128M \
  -display none -monitor none -serial stdio \
  -kernel "target/riscv64gc-unknown-none-elf/$profile/zero-to-os" >"$out" 2>&1 </dev/null &
pid=$!
sleep 2
kill "$pid" 2>/dev/null || true
cat "$out"
for expected in "$@"; do
  grep -qF -- "$expected" "$out" || { echo "없음: $expected" >&2; exit 1; }
done
```

- [ ] **2단계: 실패 확인**

실행: `scripts/check.sh "AAAA"`
기대: 실패 (`Cargo.toml`이 없어 `cargo build` 오류)

- [ ] **3단계: 빌드 설정 작성**

- `rust-toolchain.toml`: `channel = "stable"`, `targets = ["riscv64gc-unknown-none-elf"]`, `components = ["llvm-tools"]`
- `Cargo.toml`: `name = "zero-to-os"`, `version = "0.1.0"`, `edition = "2024"`, 의존성 없음
- `.cargo/config.toml`:
  - `[build] target = "riscv64gc-unknown-none-elf"`
  - `[target.riscv64gc-unknown-none-elf] runner = "qemu-system-riscv64 -machine virt -bios none -smp 4 -m 128M -nographic -kernel"`
- `build.rs`: `cargo:rustc-link-arg=-T<CARGO_MANIFEST_DIR>/kernel.ld`와 `cargo:rerun-if-changed=kernel.ld` 출력
- `.gitignore`: `/target`

- [ ] **4단계: 링커 스크립트 작성** — `kernel.ld`

- `OUTPUT_ARCH(riscv)`, `ENTRY(_start)`, 시작 주소 `0x80000000`
- 섹션 순서: `.text`(맨 앞에 `KEEP(*(.text.entry))`) → `.rodata` → `.data`(`.sdata` 포함) → `.bss`(`.sbss` 포함) → 부트 스택
- `.bss` 앞뒤에 `__bss_start`, `__bss_end` (8바이트 정렬)
- 부트 스택: `__stack_start`(16바이트 정렬)부터 코어당 64KiB × 4개

- [ ] **5단계: 진입 코드 작성**

- `src/entry.S`: 섹션 `.text.entry`, 전역 심볼 `_start`. UART 송신 레지스터(`0x10000000`)에 `'A'`를 한 바이트 쓰고 `wfi` 무한 루프. 스택을 쓰지 않는다.
- `src/main.rs`: `#![no_std]`, `#![no_main]`, `core::arch::global_asm!(include_str!("entry.S"))`, `wfi` 루프만 도는 `#[panic_handler]`.

- [ ] **6단계: 통과 확인**

실행: `scripts/check.sh "AAAA"`
기대: 종료 코드 0. 코어 4개가 각자 `A`를 하나씩 쓴다.
추가: `SMP=1 scripts/check.sh "A"`의 출력에는 `A`가 하나만 있다.

- [ ] **7단계: 링커 스크립트 변경 반영 확인**

`kernel.ld`의 시작 주소를 잠시 `0x80001000`으로 바꾸고 `scripts/check.sh "AAAA"` 실행.
기대: 다시 링크되고 실패한다. 확인 후 `0x80000000`으로 되돌리고 다시 통과하는지 확인.

- [ ] **8단계: 커밋**

```bash
git add rust-toolchain.toml Cargo.toml Cargo.lock .cargo/config.toml build.rs kernel.ld src scripts .gitignore
git commit -m "빌드 뼈대와 진입 코드 추가: 코어마다 어셈블리에서 글자 하나 출력"
```

**학습 노트** (`zero-to-os/01 부팅과 진입 코드.md`)**:**
- `no_std`, `no_main`이 필요한 이유
- QEMU 전원 직후 흐름: 리셋 벡터 `0x1000` → `0x80000000`, 그리고 모든 코어가 동시에 같은 코드를 실행한다는 것(`A`가 4개 나오는 이유)
- 링커 스크립트가 하는 일과 `_start`가 맨 앞에 와야 하는 이유
- 메모리 맵 입출력(MMIO): 주소에 쓰는 것만으로 글자가 나오는 이유

---

### Task 2: 코어별 스택, 대기, Rust 진입, UART 드라이버, `println!`

**파일:** 수정 `src/entry.S`, `src/main.rs` / 생성 `src/uart.rs`

**인터페이스:**
- 사용하는 것: `__bss_start`, `__bss_end`, `__stack_start` (태스크 1)
- 만드는 것:
  - `#[unsafe(no_mangle)] #[unsafe(link_section = ".data")] static BOOT_READY: AtomicUsize` (`src/main.rs`, 초깃값 0)
  - `#[unsafe(no_mangle)] extern "C" fn start(hartid: usize) -> !` (`src/main.rs`, M 모드)
  - `uart::init()`, `uart::putc(byte: u8)`, 매크로 `print!`, `println!`

- [ ] **1단계: 실패 확인**

실행: `scripts/check.sh "[M] hart 0: mstatus=0x"`
기대: 실패 (출력은 `AAAA`뿐)

- [ ] **2단계: `_start` 수정** — `src/entry.S`

글자 출력을 지우고 다음 순서로 바꾼다.

1. `mhartid`를 읽는다. 4 이상이면 `wfi` 루프로 멈춘다.
2. `sp = __stack_start + (hartid + 1) × 64KiB`
3. 0번 코어: `__bss_start`부터 `__bss_end`까지 0으로 채운다.
4. 나머지 코어: `BOOT_READY`가 0이 아닐 때까지 읽으며 돈다.
5. `a0 = hartid`로 `call start`. 돌아오면 `wfi` 루프.

- [ ] **3단계: UART 드라이버 작성** — `src/uart.rs`

- 베이스 주소 `0x1000_0000`, 레지스터 오프셋: THR 0, IER 1, FCR 2, LCR 3, LSR 5
- `init()`: IER=`0x00` → LCR=`0x80` → 오프셋 0=`0x03`, 오프셋 1=`0x00`(38400bps) → LCR=`0x03`(8비트, 패리티 없음) → FCR=`0x07`
- `putc()`: LSR의 비트 5가 설 때까지 기다린 뒤 THR에 쓴다.
- 레지스터 접근은 `read_volatile`/`write_volatile`을 쓴다.
- `core::fmt::Write`를 구현한 타입으로 `print!`, `println!`을 만든다. 이 태스크에서는 0번 코어만 출력하므로 락이 없다.

- [ ] **4단계: `start()`와 패닉 핸들러 작성** — `src/main.rs`

- `start(hartid)`:
  - 0번 코어: `uart::init()` → `mstatus`를 읽어 `println!("[M] hart {}: mstatus={:#x}", hartid, mstatus)` → `BOOT_READY`에 1 저장(`Ordering::Release`)
  - 모든 코어: `wfi` 루프
- 패닉 핸들러: `println!("커널 패닉: {}", info)` → `wfi` 루프

- [ ] **5단계: 통과 확인 (디버그, 릴리스)**

실행: `scripts/check.sh "[M] hart 0: mstatus=0x"`, `scripts/check.sh --release "[M] hart 0: mstatus=0x"`
기대: 둘 다 종료 코드 0

- [ ] **6단계: `BOOT_READY`가 `.bss` 밖에 있는지 확인**

```bash
"$(rustc --print sysroot)"/lib/rustlib/*/bin/llvm-nm -n target/riscv64gc-unknown-none-elf/debug/zero-to-os \
  | grep -E 'BOOT_READY|__bss_start|__bss_end'
```

기대: `BOOT_READY`의 주소가 `__bss_start`보다 작다.

- [ ] **7단계: M 모드 패닉 확인**

`start()`의 `println!` 다음 줄에 잠시 `panic!("시험")`을 넣고 `scripts/check.sh "커널 패닉: " "시험"` 실행.
기대: 종료 코드 0. 확인 후 제거.

- [ ] **8단계: 커밋**

```bash
git add src
git commit -m "코어별 스택과 Rust 진입, UART 출력 추가"
```

**학습 노트** (`zero-to-os/02 코어별 스택과 Rust 진입.md`)**:**
- Rust 코드를 부르기 전에 스택이 있어야 하는 이유, 코어마다 스택이 따로 필요한 이유
- `.bss`를 0으로 채우는 이유, 다른 코어를 기다리게 하는 이유, 대기 신호가 `.bss`에 있으면 안 되는 이유
- UART와 레지스터, `volatile` 접근이 필요한 이유
- `println!`이 `core::fmt::Write` 위에서 동작하는 방식
- CSR이 무엇인지, `mstatus` 값의 의미

---

### Task 3: `SpinLock`과 여러 코어의 동시 출력

**파일:** 생성 `src/spinlock.rs` / 수정 `src/uart.rs`, `src/main.rs`

**인터페이스 (만드는 것):**
- `pub struct SpinLock<T>` — `pub const fn new(value: T) -> Self`, `pub fn lock(&self) -> SpinLockGuard<'_, T>`
- `SpinLockGuard<'_, T>` — `Deref`/`DerefMut`, `Drop`에서 잠금 해제
- `uart::print_unlocked(args: core::fmt::Arguments)` — 락을 거치지 않는 출력(패닉 전용)
- `print!`/`println!`은 호출 한 번 전체를 락 안에서 출력한다.

- [ ] **1단계: 실패 확인**

실행: `scripts/check.sh "[M] hart 1: mstatus=0x" "[M] hart 2: mstatus=0x" "[M] hart 3: mstatus=0x"`
기대: 실패 (0번 코어만 출력)

- [ ] **2단계: 락 없이 모든 코어가 출력 (임시 실험)**

`start()`에서 모든 코어가 `BOOT_READY` 이후 `[M] hart N: mstatus=...` 줄을 100번씩 출력하게 하고 실행한다.

```bash
scripts/check.sh "[M] hart 3" | grep -c -v -x -E '\[M\] hart [0-3]: mstatus=0x[0-9a-f]+'
```

기대: 0보다 큰 수가 나올 수 있다(줄이 섞임). 실행마다 다르므로 세 번 돌려 본다. 결과를 학습 노트에 인용한다. 섞이지 않더라도 다음 단계로 간다.

- [ ] **3단계: `SpinLock<T>` 작성** — `src/spinlock.rs`

`AtomicBool` 하나로 구현한다. 획득은 `compare_exchange_weak(false, true, Acquire, Relaxed)`가 성공할 때까지 `core::hint::spin_loop()`로 돈다. 해제는 `store(false, Release)`. `unsafe impl<T: Send> Sync for SpinLock<T>`.

- [ ] **4단계: UART를 락으로 보호**

- `src/uart.rs`: 출력 타입을 `static` `SpinLock` 안에 두고, `print!`/`println!`이 락을 잡은 채 포맷 전체를 출력한다. `print_unlocked`를 추가한다.
- `src/main.rs`: 패닉 핸들러가 `print_unlocked`를 쓰게 바꾼다. `start()`는 모든 코어가 `[M] hart N: mstatus=...`를 한 번씩 출력한다(2단계의 100번 반복은 제거).

- [ ] **5단계: 통과 확인 (디버그, 릴리스)**

실행: 2단계의 반복 출력을 잠시 되살린 상태로 2단계 명령을 세 번 실행.
기대: 매번 0. 확인 후 반복을 제거하고 1단계 명령과 `--release`를 붙인 같은 명령을 실행해 종료 코드 0을 확인.

- [ ] **6단계: 락을 쥔 채 패닉 확인**

`start()`에서 잠시 UART 락을 잡은 상태로 `panic!("시험")`을 일으키고 `scripts/check.sh "커널 패닉: " "시험"` 실행.
기대: 종료 코드 0. 확인 후 제거.

- [ ] **7단계: 커밋**

```bash
git add src
git commit -m "SpinLock 추가: 여러 코어의 UART 출력이 섞이지 않게 보호"
```

**학습 노트** (`zero-to-os/03 스핀락과 동시 출력.md`)**:**
- 락이 없을 때 출력이 섞이는 이유(2단계 실험 결과 인용)
- 스핀락의 동작: 원자적 비교-교환이 필요한 이유, 읽고 나서 쓰면 안 되는 이유
- 메모리 순서(`Acquire`, `Release`)의 의미
- Rust에서 락이 데이터를 감싸는 방식(`SpinLock<T>`, 가드, `Sync`)과 그 이점
- 패닉 때 락을 건너뛰는 이유
- 백엔드에서 쓰는 뮤텍스와의 차이, 스핀락은 언제 쓰는지

---

### Task 4: M 모드에서 S 모드로 전환, `kmain()`

**파일:** 수정 `src/main.rs`

**인터페이스 (만드는 것):**
- `extern "C" fn kmain() -> !` (`src/main.rs`, S 모드)
- `fn hartid() -> usize` — `tp` 레지스터를 읽는다.

- [ ] **1단계: 실패 확인**

실행: `scripts/check.sh "[S] hart 0 시작" "[S] hart 1 시작" "[S] hart 2 시작" "[S] hart 3 시작"`
기대: 실패

- [ ] **2단계: `start()`에 모드 전환 추가**

`[M] hart N: ...` 출력은 0번 코어만 남긴다. 모든 코어가 `wfi` 루프 대신 다음을 순서대로 수행한다. CSR 쓰기는 `asm!`으로 한다.

| 순서 | 대상 | 값 | 이유 |
|---|---|---|---|
| 1 | `mstatus` | 비트 12:11(MPP)을 `0b01`로 | `mret` 후 S 모드가 되도록 |
| 2 | `mepc` | `kmain`의 주소 | `mret`가 점프할 곳 |
| 3 | `satp` | `0` | 페이징 끔 |
| 4 | `medeleg`, `mideleg` | `0xffff` | 예외·인터럽트를 S 모드로 위임 |
| 5 | `pmpaddr0` | `0x3f_ffff_ffff_ffff` | S 모드에 전체 메모리 허용 |
| 6 | `pmpcfg0` | `0xf` | 읽기·쓰기·실행, TOR 방식 |
| 7 | `tp` | `hartid` | S 모드에서 코어 번호를 알기 위해 |
| 8 | — | `mret` 실행 | S 모드 진입 |

- [ ] **3단계: `kmain()` 작성**

`println!("[S] hart {} 시작", hartid())` → `wfi` 루프

- [ ] **4단계: 통과 확인 (디버그, 릴리스)**

실행: 1단계 명령, 그리고 `--release`를 붙인 같은 명령
기대: 둘 다 종료 코드 0

줄이 섞이지 않았는지 확인:

```bash
scripts/check.sh "[S] hart 3 시작" | grep -c -x -E '\[S\] hart [0-3] 시작'
```

기대: `4`

- [ ] **5단계: 코어 수가 다를 때 확인**

- `SMP=1 scripts/check.sh "[S] hart 0 시작" | grep -c -x -E '\[S\] hart [0-3] 시작'` → `1`
- `SMP=8 scripts/check.sh "[S] hart 3 시작" | grep -c -E 'hart [0-9]+ 시작'` → `4`

- [ ] **6단계: S 모드임을 확인 (임시 실험)**

`kmain()`의 `println!` 다음 줄에 잠시 `asm!("csrr {0}, mstatus", out(reg) _)`를 넣고 빌드한 뒤 실행:

```bash
qemu-system-riscv64 -machine virt -bios none -smp 1 -m 128M -display none -monitor none -serial stdio \
  -d int -D target/int.log -kernel target/riscv64gc-unknown-none-elf/debug/zero-to-os &
sleep 2; kill $!
grep -m1 illegal_instruction target/int.log
```

기대: `illegal_instruction` 줄이 나온다. 같은 읽기는 `start()`(M 모드)에서 예외 없이 동작했다. 확인 후 제거.

- [ ] **7단계: S 모드 패닉 확인**

`kmain()`에 잠시 `panic!("시험")`을 넣고 `scripts/check.sh "커널 패닉: " "시험"` 실행.
기대: 종료 코드 0. 확인 후 제거.

- [ ] **8단계: 최종 확인과 커밋**

`git status`로 임시 코드가 남지 않았는지 확인하고 1단계 명령을 다시 실행한다.

```bash
git add src
git commit -m "S 모드 전환 추가: 모든 코어가 mret로 kmain() 진입"
```

**학습 노트** (`zero-to-os/04 M 모드에서 S 모드로.md`)**:**
- M/S/U 권한 레벨과 커널이 S 모드에서 도는 이유
- `mret`가 "돌아가는" 명령인데 모드 전환에 쓰이는 원리 (`mstatus.MPP`, `mepc`)
- CSR이 코어마다 따로 있다는 것, 그래서 모든 코어가 각자 설정해야 하는 이유
- 위임(`medeleg`, `mideleg`)과 PMP가 없으면 어떻게 되는지
- 코어 번호를 `tp`에 두는 이유, 프로그램이 자기 권한 레벨을 직접 읽을 수 없는 이유와 6단계 실험의 의미
- 다음 단계(트랩·인터럽트, 타이머) 예고
