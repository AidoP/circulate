use std::io::{Error, Read as StdRead, StdinLock, StdoutLock, Write as StdWrite};

use circulate::{BufStream, Read, Write};

pub struct IoStream<'a> {
    input: StdinLock<'a>,
    output: StdoutLock<'a>,
}
impl<'a> Read for IoStream<'a> {
    type Error = Error;
    fn read(&mut self, buffer: &mut [std::mem::MaybeUninit<u8>]) -> Result<usize, Self::Error> {
        // Safety:
        // - `0` is a valid value for u8.
        unsafe {
            buffer.as_mut_ptr().write_bytes(0, buffer.len());
            self.input.read(core::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, buffer.len()))
        }
    }
}
impl<'a> Write for IoStream<'a> {
    type Error = Error;
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.output.flush()
    }
    fn write(&mut self, slice: &[u8]) -> Result<usize, Self::Error> {
        self.output.write(slice)
    }
}

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    let mut stream = BufStream::with_capacity(IoStream {
        input: stdin.lock(),
        output: stdout.lock(),
    }, 512);

    let mut buffer = [0; 4096];
    let len = stream.read(unsafe { core::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut _, buffer.len()) }).unwrap();
    std::io::stdout().write_all(&buffer[..len]).unwrap();
    std::io::stdout().flush().unwrap();
}
