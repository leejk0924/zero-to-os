// 여러 코어가 같은 자원을 동시에 쓰지 못하게 하는 스핀락.
// 운영체제가 없어 잠든 코어를 깨워 줄 스케줄러가 없으므로, 락이 풀릴 때까지 제자리에서 돌며 기다린다.

use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

// 락이 보호할 데이터를 직접 품는다. 데이터에 닿는 길이 lock()이 돌려주는 가드뿐이라
// 락을 잡지 않고 데이터를 건드리는 코드는 컴파일되지 않는다.
pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

// UnsafeCell이 들어 있으면 컴파일러는 여러 코어가 공유해도 안전하다고 보지 않는다(Sync가 아니다).
// 락이 한 번에 한 코어만 데이터에 닿게 보장하므로 직접 Sync를 약속한다.
// 데이터가 다른 코어로 넘어가 쓰이므로 T는 Send여야 한다.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    // static에 넣으려면 컴파일 시점에 만들 수 있어야 하므로 const fn이다.
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        // "false인지 읽고 나서 true를 쓰기"로 나누면 두 코어가 같은 순간 false를 읽고 둘 다 락을 얻는다.
        // 비교와 쓰기를 한 번의 원자적 명령으로 해야 한 코어만 성공한다.
        // Acquire: 락을 얻은 뒤의 읽기·쓰기가 락을 얻기 전으로 당겨지지 않게 한다.
        // 실패했을 때는 아무것도 얻지 않았으므로 Relaxed면 된다.
        // weak는 값이 맞아도 가끔 실패할 수 있지만 어차피 반복하므로 문제없다.
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // 기다리는 중이라고 CPU에 알려 준다. 스핀 루프임을 아는 CPU는 전력이나 파이프라인을 아낄 수 있다.
            spin_loop();
        }
        SpinLockGuard { lock: self }
    }
}

// 가드가 살아 있는 동안만 데이터에 닿을 수 있고, 가드가 사라지면 락이 풀린다.
// 해제를 잊거나 두 번 하는 실수가 구조적으로 생기지 않는다.
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // 가드가 있다는 것은 이 코어가 락을 쥐고 있다는 뜻이라 다른 코어가 동시에 접근할 수 없다.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        // Release: 락을 쥔 동안의 쓰기가 모두 보인 뒤에야 다음 코어가 락을 얻게 한다.
        // 다음 코어의 Acquire와 짝을 이룬다.
        self.lock.locked.store(false, Ordering::Release);
    }
}
