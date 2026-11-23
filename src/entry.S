# 모든 코어가 0x80000000에서 동시에 이 코드를 실행한다 (링커 스크립트가 맨 앞에 배치).
# 여기서 스택을 잡고 .bss를 정리해야 그 뒤의 Rust 코드가 함수 호출과 전역 변수를 쓸 수 있다.
.section .text.entry
.global _start
_start:
    # 코어 번호는 스택 위치와 역할(초기화 담당인지)을 정하는 데 쓰인다. start()의 인자로도 넘기므로 a0에 둔다.
    csrr a0, mhartid

    # 부트 스택은 4개분만 있으므로, 그 이상 번호의 코어가 들어오면 남의 메모리를 스택으로 쓰게 된다.
    li t0, 4
    bgeu a0, t0, park

    # 스택은 높은 주소에서 낮은 주소로 자라므로 sp는 자기 64KiB 영역의 끝(= 다음 영역의 시작)을 가리켜야 한다.
    # sp = __stack_start + (hartid + 1) * 64KiB
    la sp, __stack_start
    addi t0, a0, 1
    slli t0, t0, 16
    add sp, sp, t0

    bnez a0, wait_boot

    # 0번 코어만 .bss를 지운다. 여러 코어가 동시에 지우면 먼저 끝낸 코어가 쓴 값을 다른 코어가 지워 버린다.
    # 링커 스크립트가 시작과 끝을 8바이트로 맞춰 두었으므로 8바이트씩 지워도 넘치지 않는다.
    la t0, __bss_start
    la t1, __bss_end
clear_bss:
    bgeu t0, t1, enter_rust
    sd zero, 0(t0)
    addi t0, t0, 8
    j clear_bss

wait_boot:
    # 0번 코어가 .bss와 UART를 준비하기 전에 Rust 코드로 들어가면 지워지기 전의 값을 읽거나 출력이 깨진다.
    # BOOT_READY는 .data에 있으므로 .bss를 지우는 동안에도 값이 유지된다.
    la t0, BOOT_READY
1:
    ld t1, 0(t0)
    beqz t1, 1b
    # Rust 쪽 Release 저장과 짝을 이루는 획득(acquire) 장벽: 이 뒤의 읽기가 0번 코어의 초기화 결과보다 앞서 보이지 않게 한다.
    fence r, rw

enter_rust:
    call start

park:
    # 할 일이 없는 코어는 전력을 아끼며 대기한다.
    wfi
    j park
