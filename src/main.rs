// 운영체제가 없으므로 std를 쓸 수 없고, OS가 호출해 주는 main도 없다.
#![no_std]
#![no_main]

// print!/println! 매크로를 이 파일에서 쓰려면 모듈을 먼저 선언해야 한다.
#[macro_use]
mod uart;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicUsize, Ordering};

// 진입점은 어셈블리의 _start이다. 링커 스크립트가 이를 이미지 맨 앞에 놓는다.
global_asm!(include_str!("entry.S"));

// 0번 코어가 초기화를 마쳤다는 신호. 나머지 코어는 _start에서 이 값이 0이 아니게 될 때까지 기다린다.
// 0으로 시작하는 정적 변수는 보통 .bss에 놓이는데, .bss는 0번 코어가 지우는 영역이라
// 거기 두면 지우기와 신호가 겹친다. 그래서 .data에 강제로 둔다.
// 어셈블리에서 이름으로 찾아야 하므로 이름을 바꾸지 않는다(no_mangle).
#[unsafe(no_mangle)]
#[unsafe(link_section = ".data")]
static BOOT_READY: AtomicUsize = AtomicUsize::new(0);

// _start가 코어별 스택을 잡은 뒤 부른다. 아직 M 모드다.
#[unsafe(no_mangle)]
extern "C" fn start(hartid: usize) -> ! {
    if hartid == 0 {
        uart::init();
        // mstatus는 M 모드에서만 읽을 수 있는 CSR이다. 읽기에 성공하면 지금 M 모드라는 증거가 된다.
        let mstatus: usize;
        unsafe { asm!("csrr {}, mstatus", out(reg) mstatus) };
        println!("[M] hart {}: mstatus={:#x}", hartid, mstatus);
        // Release: 위의 초기화 결과가 이 저장보다 먼저 다른 코어에 보이도록 한다.
        BOOT_READY.store(1, Ordering::Release);
    }
    loop {
        unsafe { asm!("wfi") };
    }
}

// no_std에서는 패닉 처리기를 직접 정의해야 한다. 원인을 알 수 있게 메시지를 출력하고 멈춘다.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("커널 패닉: {}", info);
    loop {
        unsafe { asm!("wfi") };
    }
}
