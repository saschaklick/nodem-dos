use core::arch::asm;
use core::cmp::min;
use crate::dos::error_code::ErrorCode;

extern crate rlibc;

/// Calls int 21h with the given registers. Returns EAX, EDX and whether the carry flag was set
/// (an error). Every register the call may change is declared, so no code is needed around it.
#[inline(never)]
fn int21(eax: u32, ebx: u32, ecx: u32, edx: u32) -> (u32, u32, bool) {
    let eax_out: u32;
    let edx_out: u32;
    let carry: u32;
    unsafe {
        asm!(
            "int 0x21",
            "sbb {c:e}, {c:e}",
            c = out(reg) carry,
            inout("eax") eax => eax_out,
            in("ebx") ebx,
            in("ecx") ecx,
            inout("edx") edx => edx_out,
        );
    }
    (eax_out, edx_out, carry != 0)
}

#[allow(dead_code)]
pub struct File {
    handle: u16,
}
#[allow(dead_code)]
pub enum SeekFrom {
    Start(u32),
    End(u32),
    Current(u32),
}

impl SeekFrom {
    fn to_dos_seek_code(&self) -> u8 {
        match self {
            SeekFrom::Start(_) => 0,
            SeekFrom::End(_) => 2,
            SeekFrom::Current(_) => 1,
        }
    }

    fn to_seek_offset(&self) -> u32 {
        match self {
            SeekFrom::Start(offset) => *offset,
            SeekFrom::End(offset) => *offset,
            SeekFrom::Current(offset) => *offset,
        }
    }
}

#[allow(dead_code)]
#[allow(unused_assignments)]
impl File {
    pub fn open(filename: &str) -> Result<Self, ErrorCode> {
        // DOS PATH length limit is 66 bytes.
        let mut filename_array: [u8; 70] = [0; 70]; // To be sure of the segment
        for i in 0..min(filename_array.len() - 1, filename.len()) {
            filename_array[i] = filename.as_bytes()[i];
        }
        let filename_ptr = filename_array.as_ptr();
        let (eax, _, carry) = int21(0x3d40, 0, 0, filename_ptr as u16 as u32);
        let error_code_or_handle = eax as u16;
        if carry {
            return Err(ErrorCode::from_u8(error_code_or_handle as u8).unwrap_or(ErrorCode::UnknownError));
        }
        Ok(Self {
            handle: error_code_or_handle,
        })
    }

    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut total_bytes_read: usize = 0;
        for buffer_write_pos in 0..buffer.len() {
            let mut tmp_stack_buffer: [u8; 1] = [0; 1]; // To be sure of the segment
            let tmp_buffer_ptr = tmp_stack_buffer.as_mut_ptr();
            let (eax, _, carry) = int21(0x3f00, self.handle as u32, 1, tmp_buffer_ptr as u16 as u32);
            let error_code_or_bytes_read = eax as u16;
            if carry {
                return Err(ErrorCode::from_u8(error_code_or_bytes_read as u8).unwrap_or(ErrorCode::UnknownError));
            }
            if error_code_or_bytes_read == 0 {
                // End of file
                break;
            }
            //total_bytes_read += error_code_or_bytes_read as usize; // = 1
            total_bytes_read += 1 as usize;
            buffer[buffer_write_pos] = tmp_stack_buffer[0];
        }

        // fill the rest of the buffer with 0s
        for buffer_write_pos in total_bytes_read..buffer.len() {
            buffer[buffer_write_pos] = 0;
        }
        Ok(total_bytes_read)
    }

    // TODO check
    pub fn write(&self, buffer: &[u8]) -> Result<usize, ErrorCode> {
        let mut total_bytes_written: usize = 0;
        for buffer_read_pos in 0..buffer.len() {
            let mut tmp_stack_buffer: [u8; 1] = [0; 1]; // To be sure of the segment
            tmp_stack_buffer[0] = buffer[buffer_read_pos];
            let tmp_buffer_ptr = tmp_stack_buffer.as_ptr();
            let (eax, _, carry) = int21(0x4000, self.handle as u32, 1, tmp_buffer_ptr as u16 as u32);
            if carry {
                return Err(ErrorCode::from_u8(eax as u8).unwrap_or(ErrorCode::UnknownError));
            }
            //total_bytes_written += error_code_or_bytes_written as usize; // = 1
            total_bytes_written += 1 as usize;
        }
        Ok(total_bytes_written)
    }

    pub fn close(self) -> Result<(), ErrorCode> {
        self.close_with_ref()
    }

    fn close_with_ref(&self) -> Result<(), ErrorCode> {
        let (eax, _, carry) = int21(0x3e00, self.handle as u32, 0, 0);
        if carry {
            return Err(ErrorCode::from_u8(eax as u8).unwrap_or(ErrorCode::UnknownError));
        }
        Ok(())
    }

    /// Seek to an offset, in bytes, in a stream.
    /// Returns number of bytes from the start of the stream if success, or an error code otherwise.
    pub fn seek(&self, pos: SeekFrom) -> Result<u32, ErrorCode> {
        let offset = pos.to_seek_offset();
        let (eax, edx, carry) = int21(0x4200 | pos.to_dos_seek_code() as u32, self.handle as u32, offset >> 16, offset & 0xffff);
        if carry {
            return Err(ErrorCode::from_u8(eax as u8).unwrap_or(ErrorCode::UnknownError));
        }
        Ok(((edx & 0xffff) << 16) | (eax & 0xffff))
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = self.close_with_ref();
    }
}