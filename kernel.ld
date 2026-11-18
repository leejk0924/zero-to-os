/* QEMU virt 머신에서 -bios none으로 켜면 모든 코어가 0x80000000(RAM 시작)에서 실행을 시작한다.
   그래서 이미지의 첫 바이트가 곧 진입 코드여야 한다. */
OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
    . = 0x80000000;

    .text : {
        /* 진입 코드를 맨 앞에 고정: 다른 코드가 앞에 끼면 코어들이 엉뚱한 곳을 실행한다. */
        KEEP(*(.text.entry))
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.sdata .sdata.*)
        *(.data .data.*)
    }

    /* .bss는 파일에 내용이 없고 0으로 채워져야 하므로, 시작/끝을 알아야 부팅 코드가 직접 지울 수 있다. */
    .bss : {
        . = ALIGN(8);
        __bss_start = .;
        *(.sbss .sbss.*)
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    }

    /* 코어마다 별도 스택이 필요하다(같이 쓰면 서로의 지역 변수를 덮어쓴다). 64KiB x 4코어. */
    .stack (NOLOAD) : {
        . = ALIGN(16);
        __stack_start = .;
        . += 4 * 64 * 1024;
    }

    /DISCARD/ : {
        *(.eh_frame .eh_frame_hdr)
    }
}
