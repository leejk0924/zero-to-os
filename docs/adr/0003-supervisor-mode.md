# ADR 0003: M 모드에서 S 모드로 전환

- 상태: 채택
- 날짜: 2026-11-19
- 이슈: #1 태스크 4
- 계획: `docs/decision/plans/2026-11-14-boot-hello.md`

## 맥락

태스크 3까지 모든 코어는 M 모드의 `start()`에서 `wfi`로 멈췄다. 커널은 S 모드에서 돌아야 한다. 계획은 설정할 CSR과 값, 순서(`mstatus.MPP` → `mepc` → `satp` → `medeleg`/`mideleg` → `pmpaddr0` → `pmpcfg0` → `tp` → `mret`)와 `kmain()`, `hartid()`의 인터페이스를 정했다. `MPP`를 어떻게 바꿀지, `tp`를 어떻게 쓸지, `kmain`의 주소를 어떻게 넘길지는 정하지 않았다.

## 결정

| 결정 | 이유 | 버린 대안 |
|---|---|---|
| CSR 값과 순서, `[M] hart N` 출력을 0번 코어만 남기기, `kmain()`이 `[S] hart N 시작` 출력 후 `wfi` 루프 | 계획대로 | |
| `mstatus`는 읽어서 비트 12:11만 지우고 `01`을 넣은 뒤 다시 쓴다 | 다른 필드(`SXL`/`UXL` 등, 지금 값 `0xa00000000`)를 건드리지 않는다. Rust 식 한 줄이라 어느 비트를 바꾸는지 읽힌다 | `csrc`/`csrs` 두 명령으로 비트를 지우고 세우기(결과는 같지만 마스크 두 개를 따로 읽어야 한다), 상수를 통째로 쓰기(다른 필드를 덮어쓴다) |
| `tp`에 쓰기와 `mret`를 한 `asm!` 블록(`mv tp, {}` + `mret`, `options(noreturn)`)에 둔다 | Rust는 `tp`를 `asm!` 피연산자로 받지 않는다. 같은 블록에서 `mret`까지 끝내면 쓰고 나서 컴파일러 코드가 끼어들 틈이 없다. `noreturn`으로 `start()`의 `-> !`가 성립한다 | `tp`를 별도 `asm!`에서 쓰기(동작은 하지만 쓰기와 사용 사이가 떨어진다) |
| `kmain`의 주소는 `kmain as *const ()`로 넘긴다 | 함수를 바로 `usize`로 바꾸면 `function_casts_as_integer` 경고가 난다. `asm!`의 `in(reg)`는 포인터를 받는다 | `kmain as usize`(경고), `sym kmain`으로 어셈블리에서 `la`(코드가 길어진다) |
| `kmain`에는 `no_mangle`을 붙이지 않는다 | 어셈블리가 이름으로 찾지 않고 Rust 코드가 주소를 직접 넘긴다 | |
| 패닉 처리기는 `static PANICKING: AtomicBool`을 `swap(true, AcqRel)`해 `false`를 받은 첫 코어만 메시지를 출력하고, 나머지 코어는 출력 없이 `wfi` 루프에 들어간다 | 여러 코어가 동시에 패닉하면 락 없는 패닉 메시지끼리 섞인다. 첫 코어만 진행하는 방식은 Linux `panic()`의 `panic_cpu`, xv6의 `panicked`와 같다 | 패닉 중 다른 코어의 `putc`를 멈추기(xv6 방식, 일반 `println!`과 섞이는 것까지 막지만 범위 밖으로 둠), 패닉 출력에도 UART 락 쓰기(락을 쥔 채 패닉하면 메시지가 안 나온다, ADR 0002) |

## 결과

- S 모드 확인: `kmain()`에서 `mstatus`를 읽으면 QEMU `-d int` 로그에 `cause:2 ... desc=illegal_instruction`이 남는다. 같은 읽기가 `start()`에서는 성공한다.
- `medeleg`로 예외를 S 모드에 위임했지만 `stvec`이 0(QEMU 초기값)이라, S 모드에서 예외가 나면 주소 0으로 뛰어 `fault_fetch`(cause 1)를 끝없이 반복한다. 2단계에서 `stvec`과 트랩 처리기를 둘 때 해결된다.
- 패닉 출력은 락을 거치지 않으므로(ADR 0002) 다른 코어가 출력 중이면 글자가 섞인다. 동시 패닉끼리는 `PANICKING`으로 막았지만, 패닉 메시지와 다른 코어의 일반 `println!`이 겹치는 경우는 남는다. `kmain()`의 `println!` 다음 줄에서 모든 코어가 `panic!("시험")`하면 `scripts/check.sh "커널 패닉: " "시험"`이 디버그 10번 중 6번, 릴리스 10번 중 1번 통과했다. 실패 13번은 모두 패닉 메시지가 다른 코어의 `[S] hart N 시작` 줄과 섞인 경우다. 플래그 전(모든 코어가 메시지를 출력)에는 10번 중 6번 통과했는데, 섞인 사본이 여럿이라 그중 하나가 온전히 남을 기회가 있었기 때문이다(추론).
- `kmain()`에서 `hartid()`로 코어 번호를 얻는다. 2단계 코드도 이것을 쓴다. `tp`를 다른 용도(스레드 로컬 등)로 쓰지 않아야 한다.
