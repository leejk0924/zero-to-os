// 운영체제가 없으므로 std를 쓸 수 없고, OS가 호출해 주는 main도 없다.
#![no_std]
#![no_main]

use core::arch::global_asm;
use core::panic::PanicInfo;

// 진입점은 어셈블리의 _start이다. 링커 스크립트가 이를 이미지 맨 앞에 놓는다.
global_asm!(include_str!("entry.S"));

// no_std에서는 패닉 처리기를 직접 정의해야 한다. 출력 수단이 없으니 멈추기만 한다.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("wfi") };
    }
}
