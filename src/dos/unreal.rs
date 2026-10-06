use core::arch::asm;

#[repr(C, align(8))]
struct Gdt([u64; 2]);

static GDT: Gdt = Gdt([0, 0x00CF_9200_0000_FFFF]);
const FLAT_DATA_SELECTOR: u16 = 0x08;

#[repr(C, packed)]
struct GdtDescriptor {
    limit: u16,
    base: u32,
}

pub fn enter() -> Result<(), &'static str> {    
    let msw: u16;
    unsafe { asm!("smsw {0:x}", out(reg) msw); }
    if msw & 1 != 0 {
        return Err("CPU is in V86 mode (EMM386/JEMM/Windows?), cannot enter unreal mode");
    }

    let ds: u16;
    unsafe { asm!("mov {0:x}, ds", out(reg) ds); }
    let gdt_descriptor = GdtDescriptor {
        limit: core::mem::size_of::<Gdt>() as u16 - 1,
        base: (ds as u32) * 16 + &GDT as *const Gdt as u32,
    };

    unsafe {
        asm!(
            "pushf",
            "cli",
            "push ds",
            "push es",
            "lgdt [{gdt}]",
            "mov eax, cr0",
            "or al, 1",
            "mov cr0, eax",
            "jmp 2f",
            "2:",
            "mov bx, {sel}",
            "mov ds, bx",
            "mov es, bx",
            "and al, 0xFE",
            "mov cr0, eax",
            "jmp 3f",
            "3:",
            "pop es",
            "pop ds",
            "popf",
            gdt = in(reg) &gdt_descriptor as *const GdtDescriptor,
            sel = const FLAT_DATA_SELECTOR,
            out("eax") _,
            out("bx") _,
        );
    }
    Ok(())
}
