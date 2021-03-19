// ensure_no_std/src/main.rs
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[allow(unused_imports)]
use epub;

use core::{alloc::GlobalAlloc, alloc::Layout, cell::UnsafeCell, panic::PanicInfo, ptr};

// Bump pointer allocator for *single* core systems
struct BumpPointerAlloc {
    head: UnsafeCell<usize>,
    end: usize,
}

unsafe impl Sync for BumpPointerAlloc {}

unsafe impl GlobalAlloc for BumpPointerAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let head = self.head.get();
        let size = layout.size();
        let align = layout.align();
        let align_mask = !(align - 1);

        // move start up to the next alignment boundary
        let start = (*head + align - 1) & align_mask;

        if start + size > self.end {
            // a null pointer signal an Out Of Memory condition
            ptr::null_mut()
        } else {
            *head = start + size;
            start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {
        // this allocater never deallocates memory
    }
}

// Declaration of the global memory allocator
// NOTE the user must ensure that the memory region `[0x2000_0100, 0x2000_0200]`
// is not used by other parts of the program
#[global_allocator]
static HEAP: BumpPointerAlloc = BumpPointerAlloc {
    head: UnsafeCell::new(0x2000_0100),
    end: 0x2000_0200,
};

#[alloc_error_handler]
fn on_oom(_layout: Layout) -> ! {
    loop {}
}

/// this function is called on panic
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
