use std::mem::MaybeUninit;

use circulate::{BufStream, Read, Write};

fn main() {
    let mut stream = BufStream::with_capacity(std::net::TcpStream::connect("localhost:8001").unwrap(), 512);

    let mut buffer: [MaybeUninit<u8>; 4096] = unsafe { MaybeUninit::uninit().assume_init() };
    while let Ok(len @ 1..) = stream.read(&mut buffer) {
        println!("{len}");
        let string = core::str::from_utf8(unsafe {
            core::slice::from_raw_parts(buffer.as_ptr() as *const u8, len)
        });
        println!("recv: {string:?}")
    }
}
