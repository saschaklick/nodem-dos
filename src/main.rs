#![no_std]
#![no_main]

extern crate alloc;

use core::str;
use core::{ arch::asm, alloc::Layout};
use alloc::string::ToString;
use alloc::{alloc::alloc};
use nodem_dos::dos::file::{self, File};
use nodem_dos::{print, println};
use nodem_dos::*;
use nodem_rs::{Area, Point, Size, PosX, PosY, Color};
use nodem_rs::media::{self, Media, Identifier};
use nodem_rs::media::font::{Alignment};
use nodem_rs::surface::{Surface};

#[cfg(feature = "vm")]
use virtmach::{VirtMach, RuntimeError, interrupt_set};

#[cfg(feature = "vm")]
interrupt_set!(interrupts = [math, proc, string, time => _, surface => _]);

/// The time interrupt of this runtime. The main loop paces the frames and the program yields
/// with HLT once per frame, so waiting is not needed: the wait functions return at once.
#[cfg(feature = "vm")]
struct DosTime;

#[cfg(feature = "vm")]
impl virtmach::interrupts::SoftInterrupt for DosTime {
    fn name(&self) -> &str { virtmach::interrupts::time::NAME }

    fn call(&mut self, vm: &mut VirtMach) {
        match vm.stack_pop() {
            // wait_for (seconds,ms)
            0 | 3 => { vm.stack_pop(); vm.stack_pop(); }
            // wait_until (seconds,ms)->(waited)
            1 => { vm.stack_pop(); vm.stack_pop(); vm.stack_push(0); }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

entry!(main);

#[cfg(not(any(nodem_mode = "index1", nodem_mode = "rgb332", nodem_mode = "rgb666")))]
compile_error!("unsupported nodem_mode");

pub struct Example {
    #[cfg(feature = "dom")]
    pub dom: nodem_rs::dom::DOM
}
impl Example {
    /// Builds the example on the heap. A function of its own, so the temporary that holds it
    /// before it is moved there is freed again and not part of the frame of main.
    #[inline(never)]
    pub fn new_on_heap() -> &'static mut Self {
        unsafe {
            let ptr = alloc(Layout::new::<Example>()) as *mut Example;
            assert!(!ptr.is_null(), "no memory for the DOM");
            ptr.write(Example::new());
            &mut *ptr
        }
    }

    #[inline(always)]
    pub fn new() -> Self {
        return Self {
            #[cfg(feature = "dom")]
            dom: Default::default()
        }
    }
}

impl Example {
    pub fn init(self: &mut Self, surface: &mut Surface) {
        const PKGS: [&str;3] = ["SYS.PKG", "INT.PKG", "EXT.PKG"];        
        let mut i = 0;
        for pkg in PKGS {        
            let _ = load_pkg(pkg, i, &mut surface.media);                        
            i += 1;            
        }              

        #[cfg(feature = "dom")]
        {
            #[cfg(feature = "xml")]
            {
                const PAGE: &str = "PAGE.XML";
                let _ = load_file(PAGE, 1024).inspect(|buf|{            
                    self.dom.from_xml(unsafe { str::from_utf8_unchecked(buf) });
                }).inspect_err(|_|{
                    self.dom.from_xml("<body vertical width=flex height=flex><div flex=1></div><div border=rect padding=4 align=center>PAGE.XML error</div><div flex=1></div></body>");
                });            
            }

            //self.dom.from(surface.media.get_page(200).content);
        }
    }

	pub fn repeat(self: &mut Self, _loop_cnt: u32, _frame_cnt: u32) {
		#[cfg(feature = "dom")]
        {
            let _dom = &mut self.dom;
        }
	}
}

fn main() {
    const MODE: GraphicsMode = GraphicsMode::Mode13;    

    #[cfg(any(nodem_mode = "rgb332", nodem_mode = "rgb666"))]
    if let GraphicsMode::Mode12 = MODE {
        println!("MODE12 does not support rgb color modes. Exiting...");
        return;
    }
    
    println!("* Nodem init ({:?})", MODE);

    let frame_size = match MODE { GraphicsMode::Mode12 => Size { width: 640, height: 480 }, GraphicsMode::Mode13 => Size { width: 320, height: 200 } };
    let frame_len = frame_size.width as usize * frame_size.height as usize * Color::BITS_PER_PIXEL / 8;
    let frame_ptr = unsafe { alloc(Layout::array::<u8>(frame_len).unwrap()) };          
    let frame = unsafe { core::slice::from_raw_parts_mut(frame_ptr, frame_len ) };

    println!("* Allocated {} bytes for framebuffer", frame_len);    
    
    #[cfg(nodem_mode = "rgb666")]
    let mut palette_len:usize = 0;
    #[cfg(nodem_mode = "rgb666")]
    let indices = {
        let len = frame_size.width as usize * frame_size.height as usize;
        unsafe { core::slice::from_raw_parts_mut(alloc(Layout::array::<u8>(len).unwrap()), len) }
    };

	let mut surface = Surface::new(frame, frame_size.width, frame_size.height);
    #[cfg(feature = "vm")]        
    let mut vm = VirtMach::new();       

    // The DOM is a few KB: on the heap, the stack of a .com is only what is left of its 64K segment
    let example = Example::new_on_heap();

    example.init(&mut surface);	  
    
    set_mode(MODE);

    #[cfg(nodem_mode = "rgb332")]
    if let GraphicsMode::Mode13 = MODE {
        set_rgb332_palette();
    }

    let mouse_ready: u16;
    let _mouse_button_cnt: u16;
    unsafe { asm!("int 33h", inout("ax") 0x0000u16 => mouse_ready, out("bx") _mouse_button_cnt); }

    // Big scratch buffers on the heap: the stack of a .com is only what the program leaves free of 64K
    #[cfg(nodem_mode = "rgb666")]
    let palette: &mut [u32; 256] = unsafe { &mut *(alloc(Layout::new::<[u32; 256]>()) as *mut [u32; 256]) };

    let mut loop_cnt: u32 = 0;
    let mut frame_cnt: u32 = 0;	

    #[cfg(feature = "vm")]
    {
        // the programs of EXT.PKG were compiled with the old interrupt numbers and hide the ones of INT.PKG
        surface.media.programs[2].clear();
        vm.load_program(surface.media.get_program(1));
    }

    loop {		
		example.repeat(loop_cnt, frame_cnt);		                

        #[cfg(feature = "vm")]
        {                        
            use nodem_rs::int_surface;         

            let mut int_surface = int_surface::IntSurface { surface : &mut surface };
            let mut time = DosTime;
            interrupts!(let list, &mut time, &mut int_surface);

            vm.run(8 * 1024, list);                       
            
            if vm.error != RuntimeError::NoError {
                // the program may have changed the clip and palette, the overlay needs the defaults
                surface.clip.reset(surface.width, surface.height);
                surface.palette = Color::COLORS;
                // the number of the error: its position in RuntimeError
                let code = unsafe { *(&vm.error as *const RuntimeError as *const u8) };
                let text = [b'E', b'R', b'R', b' ', 0x30 + code / 10 % 10, 0x30 + code % 10];
                surface.draw_text(Identifier::Index(0), unsafe { str::from_utf8_unchecked(&text) }, Point { x: 1, y: 1 });
            }               
        }

        #[cfg(all(feature = "dom", not(feature = "vm")))]
        {
            surface.clear(0);                   
            surface.update(&mut example.dom);                                        
        }        
        
        cfg_select! {
            nodem_mode = "rgb666" => { draw_fps(&mut surface, if loop_cnt % 20 < 10 { palette_len as u32 } else { loop_cnt }); }
            _ => { draw_fps(&mut surface, loop_cnt); }
        }

        if mouse_ready == 0xffff {
            // Get position: BX = buttons, CX = x, DX = y. X is in 640-wide virtual coordinates in mode 13h.
            let (mut x, y): (u16, u16);
            unsafe { asm!("int 33h", inout("ax") 0x0003u16 => _, out("bx") _, out("cx") x, out("dx") y); }
            if let GraphicsMode::Mode13 = MODE { x /= 2; }
            surface.draw_image(Identifier::Index(252), Point { x: x as i16, y: y as i16 }, None);
        }
		        
        match MODE {
            GraphicsMode::Mode12 => {                
                for y in 0..480u16 {
                    let row = &frame[y as usize * 80..(y as usize + 1) * 80];
                    let mut line = [0u8; 80];
                    line.copy_from_slice(row);
                    blit_to_vga(y * 80, &line);
                }
            },
            GraphicsMode::Mode13 => {                
                #[cfg(nodem_mode = "rgb666")]
                {
                    palette_len = quantize_rgb666(frame, indices, palette);
                    // the palette changes with every frame: swap it and the pixels together in the retrace
                    wait_vsync();
                    load_vga_palette(palette);
                }
                for y in 0..200usize {
                    let mut line = [0u8; 320]; // on the stack: blit_to_vga only takes a 16 bit offset
                    cfg_select! {
                        nodem_mode = "rgb332" => {
                            line.copy_from_slice(&frame[y * 320..y * 320 + 320]);
                        }
                        nodem_mode = "rgb666" => {
                            line.copy_from_slice(&indices[y * 320..y * 320 + 320]);
                        }
                        _ => {
                            for x in 0..320usize {
                                let byte = frame[y * 40 + x / 8];
                                line[x] = if byte & (1 << (7 - (x % 8))) != 0 { 15 } else { 0 };
                            }
                        }
                    }                    
                    
                    blit_to_vga(y as u16 * 320, &line);
                }
            }
        }

		loop_cnt += 1;		        
		
		if loop_cnt == match MODE { GraphicsMode::Mode12 => 640 * 480 / 8, GraphicsMode::Mode13 => 320 * 200 } {
			frame_cnt += 1;
			loop_cnt = 0;
		}
   }
}

#[allow(dead_code)]
#[derive(Debug)]
enum GraphicsMode {
    Mode12,
    Mode13
}

fn set_mode(mode: GraphicsMode) {
    match mode {        
        GraphicsMode::Mode12 => unsafe { asm!("mov ax, 0012h", "int 10h" ); },
        GraphicsMode::Mode13 => unsafe { asm!("mov ax, 0013h", "int 10h" ); },
    }    
}

#[cfg(nodem_mode = "rgb332")]
fn set_rgb332_palette() {
    unsafe {
        asm!("out dx, al", in("dx") 0x3C8u16, in("al") 0u8); // start at index 0, auto-increments
        for i in 0..=255u16 {
            let r = ((i >> 5) & 0x07) * 63 / 7;
            let g = ((i >> 2) & 0x07) * 63 / 7;
            let b = (i & 0x03) * 63 / 3;
            asm!("out dx, al", in("dx") 0x3C9u16, in("al") r as u8);
            asm!("out dx, al", in("dx") 0x3C9u16, in("al") g as u8);
            asm!("out dx, al", in("dx") 0x3C9u16, in("al") b as u8);
        }
    }
}

fn blit_to_vga(offset: u16, line: &[u8]) {
    unsafe {
        asm!(
            "push es",
            "push si",
            "mov es, {seg:x}",
            "mov si, {src:x}",
            "cld",
            "rep movsb",
            "pop si",
            "pop es",
            seg = in(reg) 0xA000u16,
            src = in(reg) line.as_ptr() as u16,
            inout("di") offset => _,
            inout("cx") line.len() as u16 => _,
        );
    }
}

#[cfg(nodem_mode = "rgb666")]
fn quantize_rgb666(frame: &[u8], indices: &mut [u8], palette: &mut [u32;256]) -> usize {
    let mut palette_len = 1usize;
    let mut last_color = 0u32;
    let mut last_index = 0u8;

    for (p, px) in frame.chunks_exact(3).zip(indices.iter_mut()) {
        let color = (p[0] as u32) | ((p[1] as u32) << 8) | ((p[2] as u32) << 16);
        if color != last_color {
            last_index = match palette[..palette_len].iter().position(|c| *c == color) {
                Some(i) => i as u8,
                None if palette_len < 256 => {
                    palette[palette_len] = color;
                    palette_len += 1;
                    (palette_len - 1) as u8
                }
                None => nearest_color(&palette[..palette_len], color),
            };
            last_color = color;
        }
        *px = last_index;
    }
    palette_len
}

#[cfg(nodem_mode = "rgb666")]
fn nearest_color(palette: &[u32], color: u32) -> u8 {
    let (mut best, mut best_dist) = (0usize, u32::MAX);
    for (i, c) in palette.iter().enumerate() {
        let mut dist = 0u32;
        for shift in [0, 8, 16] {
            let d = ((color >> shift) & 0xFF) as i32 - ((c >> shift) & 0xFF) as i32;
            dist += (d * d) as u32;
        }
        if dist < best_dist { best = i; best_dist = dist; }
    }
    best as u8
}

#[cfg(nodem_mode = "rgb666")]
/// Waits for the start of the vertical retrace of the VGA
fn wait_vsync() {
    unsafe {
        let mut status: u8;
        // if one is running already wait for it to end, so the whole retrace is ahead
        loop {
            asm!("in al, dx", in("dx") 0x3DAu16, out("al") status);
            if status & 8 == 0 { break; }
        }
        loop {
            asm!("in al, dx", in("dx") 0x3DAu16, out("al") status);
            if status & 8 != 0 { break; }
        }
    }
}

fn load_vga_palette(colors: &[u32;256]){
    unsafe {
        asm!("out dx, al", in("dx") 0x3C8u16, in("al") 0u8); // start at index 0, auto-increments
        for &c in colors.iter() {
            asm!("out dx, al", in("dx") 0x3C9u16, in("al") (c & 0x3F) as u8);
            asm!("out dx, al", in("dx") 0x3C9u16, in("al") ((c >> 8) & 0x3F) as u8);
            asm!("out dx, al", in("dx") 0x3C9u16, in("al") ((c >> 16) & 0x3F) as u8);
        }
    }
}

fn draw_fps(surface: &mut Surface, fps: u32) {
    let font_idx = 0;
    let mut text = [0u8;4];
    
    let mut zero = true;
    for i in 0..3 {
        text[i] = (0x30 + fps / 10u32.pow(2 - i as u32) % 10) as u8;
        if text[i] == 0x30 && i < 2 { if zero { text[i] = 0x20; } } else { zero = false; }
    }
    let text = unsafe { str::from_utf8_unchecked(&text) };
    // a program of the vm may have changed these
    surface.clip.reset(surface.width, surface.height);
    surface.palette = Color::COLORS;
    surface.media.typesetting = media::font::Settings::default();
    surface.media.typesetting.mono_spaced = Alignment::Center;
    surface.media.drawing = media::image::Settings::default();
        
    let mut size = surface.get_text_size(Identifier::Index(font_idx), text);
    size.width += 4;
    size.height += 4;    
    let position = Point { x: (surface.width - size.width) as PosX - 1, y: (surface.height - size.height) as PosY - 1 };    
    
    surface.fill_rect(Area{ point: position, size: size }, 0);        
    surface.draw_rect(Area{ point: position, size: size }, 1);        
    surface.draw_text(Identifier::Index(font_idx), text, Point { x: position.x + 2, y: position.y + 2 });
}

fn load_pkg <'a> (file: &'a str, index: u8, media: &'a mut Media) -> Result<&'a [u8], &'a str> {
    print!("* Loading PKG {} ... ", file);    
    
    let file_res = File::open(file);
    if file_res.is_ok() {
        let file = file_res.unwrap();
                
        let file_size: usize;
        
        let mut buf = [0; 8];
        let read_res = file.read(&mut buf);
        if read_res.is_ok() {
            if buf[0] == b'P' && buf[1] == b'K' && buf[2]== b'G' && buf[3] == b'0' {
                file_size = ((buf[4] as usize) << 0) | ((buf[5] as usize) << 8) | ((buf[6] as usize) << 16) | ((buf[7] as usize) << 24);
            }else{
                println!("malformed");
                return Err("malformed");
            }
        }else{
            return Err("failed read");
        }        
        let _ = file.seek(file::SeekFrom::Start(0));        
    
        let layout_res = Layout::array::<u8>(file_size);          
        if layout_res.is_err() {
            let _ = file.close();
            println!("layout error");
            return Err("layout error");
        }

        let ptr: *mut u8;
        let slice: &mut [u8];
        unsafe {                
            ptr = alloc(layout_res.unwrap());
            if ptr.is_null() {
                println!("alloc failed");
                return Err("alloc failed");
            } else {             
                slice = core::slice::from_raw_parts_mut(ptr, file_size);            
            }
        }

        let read_res = file.read(slice);
        let _ = file.close();

        if read_res.is_err() {
            println!("failed read");
            Err("failed read")
        }else{
            media.load_pkg(ptr, file_size, index);
            println!("OK {:-5}b at 0x{:08x}", file_size, ptr as u32);
            return Ok(slice)
        }                
    }else{        
        Err("failed open")
        //Err(file_res.err().unwrap().as_str())
    }    
}

fn load_file <'a> (file: &'a str, file_size: usize) -> Result<&'a [u8], &'a str> {
    print!("* Loading file {} ({}b) ... ", file, file_size);    
    
    let file_res = File::open(file);
    if file_res.is_ok() {        
        let file = file_res.unwrap();        
    
        let layout_res = Layout::array::<u8>(file_size);          
        if layout_res.is_err() {
            let _ = file.close();            
            return Err("layout error");
        }

        let ptr: *mut u8;
        let slice: &mut [u8];
        unsafe {                            
            ptr = alloc(layout_res.unwrap());
            if ptr.is_null() {                
                return Err("alloc failed");
            }else{         
                slice = core::slice::from_raw_parts_mut(ptr, file_size);
            }
        }        

        let read_res = file.read(slice);
        let _ = file.close();

        if read_res.is_err() {            
            Err("failed read")
        }else{            
            println!("OK {:-5}b at 0x{:08x}", file_size, ptr as u32);
            return Ok(slice)
        }
                               
    }else{
        Err(file_res.err().unwrap().as_str())
    }    
}