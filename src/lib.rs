#![no_std]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod dos;
pub mod dpkey;
extern crate rlibc;
extern crate alloc;

use crate::dos::allocator::GLOBAL_ALLOCATOR;
use crate::dos::cooperative_multitasking::TASKING;

#[link_section = ".startup"]
#[no_mangle]
fn _start() -> ! {
    if let Err(msg) = dos::unreal::enter() {
        println!("{}", msg);
        dos::exit(1);
    }
    #[allow(static_mut_refs)]
    unsafe {
        GLOBAL_ALLOCATOR.init(); // Heap lies above DS:FFFF, needs unreal mode
        TASKING.init(); // Relies on the allocator
    }
    extern "Rust" {
        fn main() -> ();
    }
    unsafe {
        main();
    }
    dos::exit(0);
}

#[macro_export]
macro_rules! entry {
    ($path:path) => {
        #[export_name = "main"]
        pub fn __main() -> () {
            // type check the given path
            let f: fn() -> () = $path;
            f()
        }
    };
}