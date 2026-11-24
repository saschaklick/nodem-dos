use core::arch::asm;
use core::fmt::{self, Write};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::dos::console::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    ($fmt:expr) => {
        print!(concat!($fmt, "\r\n"))
    };
    ($fmt:expr, $($arg:tt)*) => {
        print!(concat!($fmt, "\r\n"), $($arg)*)
    };
}

pub fn _print(args: fmt::Arguments) {
    let mut writer = DosWriter {};
    writer.write_fmt(args).unwrap();
}

struct DosWriter;

impl Write for DosWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            printc(c);
        }
        Ok(())
    }
}

fn printc(ch: u8) {
    // int 21h/02h returns the character in AL, so EAX is clobbered and has to be declared as such
    unsafe { asm!("int 0x21", inout("eax") 0x0200u32 => _, in("edx") ch as u32) }
}
