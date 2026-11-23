# ADR 0002: SpinLock과 여러 코어의 UART 출력

- 상태: 채택
- 날짜: 2026-11-19
- 이슈: #1 태스크 3
- 계획: `docs/decision/plans/2026-11-14-boot-hello.md`

## 맥락

태스크 2까지는 0번 코어만 출력했다. 모든 코어가 `println!`을 부르면 바이트가 번갈아 들어가 줄이 섞인다. 락 없이 코어마다 100줄씩 출력한 실험에서 400줄 중 형식이 깨진 줄이 399, 390, 397개였다. 계획은 `SpinLock<T>`의 구현 방식(`AtomicBool`, `compare_exchange_weak`, `Acquire`/`Release`)과 인터페이스를 정했다. 락이 무엇을 감쌀지, 락을 어디서 잡을지, 패닉 경로를 어떻게 구현할지는 정하지 않았다.

## 결정

| 결정 | 이유 | 버린 대안 |
|---|---|---|
| `SpinLock<T>`를 `AtomicBool` + `compare_exchange_weak(false, true, Acquire, Relaxed)` + `spin_loop()`로 짓고, 가드의 `Drop`에서 `store(false, Release)`로 푼다. `unsafe impl<T: Send> Sync` | 계획대로 | |
| `SpinLockGuard`에 `_marker: PhantomData<&'a T>` 필드를 둬, 가드는 `T: Send + Sync`일 때만 `Sync`다 | Codex 지적. `&SpinLock<T>`만 들면 `T: Send`만으로 가드가 `Sync`가 되어, `Cell`처럼 `Sync`가 아닌 `T`를 여러 코어가 한 가드의 `Deref`로 락 없이 함께 고칠 수 있다. std `MutexGuard`와 같은 규칙이고 `unsafe` 없이 컴파일러가 판단한다 | `PhantomData<*const ()>`로 자동 `Sync`를 끄고 `unsafe impl<T: Sync> Sync`를 직접 쓰기(같은 결과를 `unsafe` 약속으로 얻는다) |
| 락이 크기 0인 `Uart` 값을 감싼다: `static UART: SpinLock<Uart>` | 출력 경로가 `UART.lock()`이 돌려주는 가드를 거쳐야만 `write_fmt`를 부를 수 있다. 락과 보호 대상이 한 값이라 "락을 잡고 출력"이 구조로 강제된다 | `SpinLock<()>`를 따로 두고 `Uart`를 아무 데서나 만들기(락을 잊어도 컴파일된다) |
| 락은 `_print` 한 곳에서 `UART.lock().write_fmt(args)`로 잡는다 | 가드가 문장 끝까지 살아 있어 서식 처리부터 마지막 바이트까지 락 안이다. `println!`은 `print!("{}\n", ..)` 한 번이라 줄바꿈까지 한 덩어리로 나간다. 태스크 2가 출력을 `_print` 한 곳으로 모아 둔 덕에 바뀐 곳이 이 한 줄이다 | 매크로 안에서 락을 잡기(매크로가 펼쳐지는 곳마다 락 코드가 생긴다) |
| `print_unlocked`는 `Uart`를 새로 만들어 바로 쓴다. 패닉 핸들러는 `print_unlocked(format_args!("커널 패닉: {}\n", info))`를 부른다 | 락을 쥔 코어가 패닉하면 그 락은 영영 풀리지 않는다. 실험에서 패닉 핸들러를 `println!`으로 두면 메시지가 전혀 나오지 않았다. 다른 코어 출력과 섞일 위험보다 메시지가 보이는 쪽이 중요하다 | 패닉 때 락을 강제로 풀기(다른 코어가 쓰던 중이면 똑같이 섞이고, 락의 불변식을 깬다), `try_lock` 후 실패하면 건너뛰기(락을 쥔 코어 자신이 패닉하면 메시지가 안 나온다) |
| `start()`는 0번 코어의 초기화와 `BOOT_READY` 저장 뒤, 모든 코어가 각자 `mstatus`를 읽어 한 줄씩 출력한다 | 계획대로. 0번 코어도 신호를 세운 뒤에 출력하므로 네 코어가 실제로 동시에 출력하게 된다 | |

## 결과

- 릴리스 빌드의 기계어: 획득 루프는 `amoor.w.aq`(원자적 OR + acquire)와 `pause`가 되고, 해제는 `fence rw, w` 뒤 `sb zero`가 된다. 컴파일러가 `bool`의 `false → true` 비교-교환을 원자적 OR로 바꿨다. LR/SC는 나오지 않았다.
- 스핀락은 인터럽트를 끄지 않는다(설계대로). 2단계에서 인터럽트를 켜면, 락을 쥔 코어가 인터럽트 처리 중 같은 락을 잡을 때 스스로 교착된다. 그때 락을 잡는 동안 인터럽트를 끄는 처리를 더해야 한다.
- 락을 쥔 코어가 패닉하면 그 락을 기다리는 다른 코어는 영원히 돈다. 디버그 빌드 실험에서 0번 코어가 락을 쥐고 패닉하자, 아직 출력하지 못한 3번 코어의 줄이 끝내 나오지 않았다. 패닉 뒤 시스템은 어차피 멈추므로 받아들인다.
- 태스크 4는 `println!`을 S 모드에서 그대로 쓸 수 있다. 락은 M/S 모드와 무관하게 메모리 원자 연산만 쓴다.
