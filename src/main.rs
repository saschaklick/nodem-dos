#![no_std]
#![no_main]

extern crate alloc;

use core::str;
use core::{ arch::asm, alloc::Layout};
use alloc::{alloc::alloc};
use nodem_dos::dos::file::{self, File};
use nodem_dos::{print, println};
use nodem_dos::*;
use nodem_rs::{Area, Point, Size, PosX, PosY};
use nodem_rs::media::{self, Media, Identifier};
use nodem_rs::media::font::{Alignment};
use nodem_rs::surface::{Surface};
#[cfg(feature = "vm")]
use virtmach::{VirtMach, RuntimeError, interrupts::*};
#[cfg(feature = "vm")]
use nodem_rs::int_surface;

entry!(main);

pub struct Example {
    #[cfg(feature = "dom")]
    pub dom: nodem_rs::dom::DOM
}
impl Example {
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

            self.dom.from(surface.media.get_page(200).content);  
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

    println!("* Nodem init ({:?})", MODE);

    let frame_size = match MODE { GraphicsMode::Mode12 => Size { width: 640, height: 480 }, GraphicsMode::Mode13 => Size { width: 320, height: 200 } };
    let frame_len = frame_size.width as usize * frame_size.height as usize / 8;
    let frame_ptr = unsafe { alloc(Layout::array::<u8>(frame_len).unwrap()) };          
    let frame = unsafe { core::slice::from_raw_parts_mut(frame_ptr, frame_len ) };

	let mut surface = Surface::new(frame, frame_size.width, frame_size.height);	 
    #[cfg(feature = "vm")]
    let mut vm = VirtMach::new();       

	let mut example = Example::new();    

    example.init(&mut surface);	    
    
    set_mode(MODE);    

    // let mut mouse_ready = [0u16; 1];
    // let mut mouse_pos = [0u16; 2];

    //unsafe { asm!("push ax", "mov ax, 0020h", "int 33h", "mov ax, 0000h", "int 33h", out("ax") mouse_ready[0]); asm!("pop ax"); }    

    let mut loop_cnt: u32 = 0;
    let mut frame_cnt: u32 = 0;	

    #[cfg(feature = "vm")]
    {
        vm.load_program(surface.media.get_program(3));
    }

    loop {		
		example.repeat(loop_cnt, frame_cnt);		                

        #[cfg(feature = "vm")]
        {
            let mut interrupts: [&mut dyn SoftInterrupt;4] = [
                &mut proc::Interrupt {},
                &mut math::Interrupt {},
                &mut random::Interrupt {},                
                &mut int_surface::IntSurface { surface : &mut surface }
            ];

            vm.run(1024, &mut interrupts);                       
            
            if vm.error != RuntimeError::NoError {
                surface.draw_text(Identifier::Index(0), "ERROR", Point { x: 1, y: 1 });
            }            
        }

        #[cfg(feature = "dom")]
        {
            surface.clear(0);                   
            surface.update(&mut example.dom);                                        
        }        
        
        draw_fps(&mut surface, loop_cnt);                

        // if mouse_ready[0] == 0xffff {
        //     unsafe { asm!("push ax", "push bx", "push cx", "push dx"); }
        //     unsafe { asm!("mov ax, 0003h", "int 33h", out("cx") mouse_pos[0], out("dx") mouse_pos[1]); }
        //     unsafe { asm!("pop dx", "pop cx", "pop bx", "pop ax"); }

        //     //println!(">> {} {} {}", mouse_ready[0], mouse_pos[0], mouse_pos[1]);        
        
        //     surface.draw_image(Identifier::Index(252), Point{ x: mouse_pos[0] as i16, y: mouse_pos[1] as i16 }, None);           
        // }
		        
        //let pixels = (0xa0000) as *mut u8;                
        let pixels = (0xa0000 - 20 * 320 - 32) as *mut u8;                
        //let pixels = (0xa0000 - 103 * 320 - 112) as *mut u8;                

        let framebuffer: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(pixels, match MODE { GraphicsMode::Mode12 => 640 * 480 / 8, GraphicsMode::Mode13 => 320 * 200 }) };        
        
        match MODE {
            GraphicsMode::Mode12 => {
                framebuffer.copy_from_slice(frame);                
            },
            GraphicsMode::Mode13 => {                     
                //let mut segment = 0xa000u16;           
                let mut offset = 0x00u16;
                for y in 0..200 {                     
                    for x in 0..320 {                                                
                        let pixel = if (frame[(y * 320 + x) as usize / 8] & (1 << (7 - (x % 8)))) != 0 { 15 } else { 0 };                        
                        //unsafe { asm!("mov ds, {seg:x}", "mov si, {off:x}", "mov ds:[si], {pxl:x}", seg = in(reg) segment, off = in(reg) offset as u16, pxl = in(reg) pixel as u16); }
                        framebuffer[(y * 320 + x) as usize] = pixel;
                        offset += 0x1;
                        if offset == 0x10 {
                            offset = 0;
                            //segment += 0x1;                            
                        }
                    }
			    }			
            }
        }                


		loop_cnt += 1;		        
		
		if loop_cnt as usize == framebuffer.len() {
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

fn draw_fps(surface: &mut Surface, fps: u32) {
    let font_idx = 0;
    let mut text = [0u8;4];
    
    let mut zero = true;
    for i in 0..3 {
        text[i] = (0x30 + fps / 10u32.pow(2 - i as u32) % 10) as u8;
        if text[i] == 0x30 && i < 2 { if zero { text[i] = 0x20; } } else { zero = false; }
    }
    let text = unsafe { str::from_utf8_unchecked(&text) };
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
                return Err("malformed");
            }
        }else{
            return Err("failed read");
        }        
        let _ = file.seek(file::SeekFrom::Start(0));        
    
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
            } else {             
                slice = core::slice::from_raw_parts_mut(ptr, file_size);            
            }
        }

        let read_res = file.read(slice);
        let _ = file.close();

        if read_res.is_err() {
            Err("failed read")
        }else{
            media.load_pkg(ptr, file_size, index);
            println!("OK {:-5}b at 0x{:08x}", file_size, ptr as u32);
            return Ok(slice)
        }
                               
    }else{
        Err(file_res.err().unwrap().as_str())
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