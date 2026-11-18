// 운영체제가 없으므로 std를 쓸 수 없고, OS가 호출해 주는 main도 없다.
#![no_std]
#![no_main]

// print!/println! 매크로를 이 파일에서 쓰려면 모듈을 먼저 선언해야 한다.
#[macro_use]
mod uart;
mod spinlock;

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
// 커널은 S 모드에서 돌아야 하므로, 여기서 모드 전환에 필요한 CSR을 설정하고 mret로 kmain()에 들어간다.
#[unsafe(no_mangle)]
extern "C" fn start(hartid: usize) -> ! {
    if hartid == 0 {
        uart::init();
        // Release: 위의 초기화 결과가 이 저장보다 먼저 다른 코어에 보이도록 한다.
        BOOT_READY.store(1, Ordering::Release);
        // mstatus는 M 모드에서만 읽을 수 있는 CSR이다. 읽기에 성공하면 지금 M 모드라는 증거가 된다.
        let mstatus: usize;
        unsafe { asm!("csrr {}, mstatus", out(reg) mstatus) };
        println!("[M] hart {}: mstatus={:#x}", hartid, mstatus);
    }

    // CSR은 코어마다 따로 있으므로 아래 설정은 모든 코어가 각자 해야 한다.
    unsafe {
        // mret는 mstatus.MPP(비트 12:11)에 적힌 권한 레벨로 돌아간다. 01(S 모드)로 두어 mret 후 S 모드가 되게 한다.
        let mut mstatus: usize;
        asm!("csrr {}, mstatus", out(reg) mstatus);
        mstatus = (mstatus & !(0b11 << 11)) | (0b01 << 11);
        asm!("csrw mstatus, {}", in(reg) mstatus);

        // mret가 점프할 주소. "돌아갈 곳"을 kmain으로 꾸며 둔다.
        asm!("csrw mepc, {}", in(reg) kmain as *const ());

        // 페이징은 아직 쓰지 않는다. 0이면 S 모드의 주소가 그대로 물리 주소다.
        asm!("csrw satp, zero");

        // 예외와 인터럽트를 S 모드가 직접 처리하도록 위임한다. 위임하지 않으면 모두 M 모드로 올라간다.
        asm!("csrw medeleg, {}", in(reg) 0xffff);
        asm!("csrw mideleg, {}", in(reg) 0xffff);

        // PMP 규칙이 하나도 없으면 S 모드는 어떤 메모리에도 접근할 수 없다.
        // 항목 0을 TOR(0부터 pmpaddr0까지) + 읽기·쓰기·실행으로 두어 전체 메모리를 연다.
        asm!("csrw pmpaddr0, {}", in(reg) 0x3f_ffff_ffff_ffff_usize);
        asm!("csrw pmpcfg0, {}", in(reg) 0xf);

        // S 모드에서는 mhartid를 읽을 수 없으므로 코어 번호를 tp에 남겨 둔다.
        // tp는 컴파일러가 쓰지 않는 레지스터라 mret 뒤에도 값이 유지된다.
        // tp는 asm! 피연산자로 지정할 수 없어 mv로 옮기고, mret까지 한 블록에서 끝낸다.
        asm!("mv tp, {}", "mret", in(reg) hartid, options(noreturn));
    }
}

// S 모드 커널의 진입점. start()가 mepc에 이 주소를 넣고 mret로 뛰어 온다.
extern "C" fn kmain() -> ! {
    println!("[S] hart {} 시작", hartid());
    loop {
        unsafe { asm!("wfi") };
    }
}

// start()가 tp에 넣어 둔 코어 번호를 읽는다.
fn hartid() -> usize {
    let id: usize;
    unsafe { asm!("mv {}, tp", out(reg) id) };
    id
}

// no_std에서는 패닉 처리기를 직접 정의해야 한다. 원인을 알 수 있게 메시지를 출력하고 멈춘다.
// UART 락을 쥔 채 패닉했을 수 있으므로 println! 대신 락을 거치지 않는 출력을 쓴다.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart::print_unlocked(format_args!("커널 패닉: {}\n", info));
    loop {
        unsafe { asm!("wfi") };
    }
}
