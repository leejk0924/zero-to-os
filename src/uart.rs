// QEMU virt 머신의 NS16550A UART 드라이버. 레지스터는 메모리 주소로 노출된다(MMIO).

use core::fmt::{self, Write};
use core::ptr::{read_volatile, write_volatile};

const BASE: usize = 0x1000_0000;

// 레지스터 오프셋. LCR의 DLAB 비트가 서 있으면 0, 1번은 보율 분주값(DLL, DLM)이 된다.
const THR: usize = 0; // 송신 버퍼
const IER: usize = 1; // 인터럽트 허용
const FCR: usize = 2; // FIFO 제어
const LCR: usize = 3; // 회선 제어
const LSR: usize = 5; // 회선 상태

// LSR 비트 5: 송신 버퍼가 비어 다음 바이트를 받을 수 있다.
const LSR_THR_EMPTY: u8 = 1 << 5;

// 일반 메모리 접근이면 컴파일러가 반복 읽기를 하나로 합치거나 쓰기를 없앨 수 있다.
// 장치 레지스터는 읽고 쓰는 것 자체가 동작이므로 매번 실제로 접근하도록 volatile을 쓴다.
fn write_reg(offset: usize, value: u8) {
    unsafe { write_volatile((BASE + offset) as *mut u8, value) }
}

fn read_reg(offset: usize) -> u8 {
    unsafe { read_volatile((BASE + offset) as *const u8) }
}

// 실제 보드에서는 전원 직후 UART 설정이 정해져 있지 않으므로 통신 형식을 직접 맞춘다.
pub fn init() {
    // 아직 트랩 처리가 없으므로 UART 인터럽트가 오면 안 된다.
    write_reg(IER, 0x00);
    // DLAB을 켜야 0, 1번 오프셋으로 보율 분주값을 쓸 수 있다.
    write_reg(LCR, 0x80);
    write_reg(0, 0x03); // DLL: 분주값 3 → 38400bps
    write_reg(1, 0x00); // DLM
    // DLAB을 끄고 8비트, 패리티 없음, 정지 비트 1로 정한다.
    write_reg(LCR, 0x03);
    // FIFO를 켜고 송수신 FIFO를 비운다.
    write_reg(FCR, 0x07);
}

// 송신 버퍼가 차 있을 때 쓰면 앞 글자를 덮어써 글자가 사라지므로 빌 때까지 기다린다.
pub fn putc(byte: u8) {
    while read_reg(LSR) & LSR_THR_EMPTY == 0 {}
    write_reg(THR, byte);
}

// core::fmt::Write를 구현하면 format_args!가 만든 서식 문자열을 그대로 출력할 수 있다.
struct Uart;

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            putc(byte);
        }
        Ok(())
    }
}

// 매크로가 다른 모듈에서 펼쳐져도 부를 수 있어야 하므로 pub이다. 직접 부르지 않고 print!를 쓴다.
// 지금은 0번 코어만 출력하므로 락이 없다. 여러 코어가 동시에 쓰면 글자가 섞인다.
pub fn _print(args: fmt::Arguments) {
    let _ = Uart.write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::uart::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {
        $crate::print!("{}\n", format_args!($($arg)*))
    };
}
