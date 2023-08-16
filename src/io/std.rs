use core::mem::MaybeUninit;
use std::io;

impl<T: io::Read> super::Read for T {
    type Error = io::Error;
    fn read(&mut self, buffer: &mut [MaybeUninit<u8>]) -> Result<usize, Self::Error> {
        // Currently there is no stable way to read in to an uninitialised buffer so pointlessly initlialise it.
        // Safety:
        // - A slice upholds the guarantees of write_bytes and 0 is a valid bit pattern for `MaybeUninit<u8>`.
        // - After writing all 0's it is safe to reconstruct the slice as initialised.
        unsafe {
            core::ptr::write_bytes(buffer.as_mut_ptr(), 0, buffer.len());
            <T as io::Read>::read(self, core::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, buffer.len()))
        }
    }
    fn read_vectored(&mut self, buffers: &mut [super::IoVecMut]) -> Result<usize, Self::Error> {
        // Currently there is no stable way to read in to an uninitialised buffer so pointlessly initlialise it.
        // Safety:
        // - A slice upholds the guarantees of write_bytes and 0 is a valid bit pattern for `MaybeUninit<u8>`.
        // - After writing all 0's it is safe to reconstruct the slice as initialised IoSliceMut's.
        unsafe {
            // TODO: use syslib IoVecMut's which could be made to allow a no-op conversion to std's IoSliceMut.
            let mut buffers: Vec<_> = buffers.into_iter().map(|buffer| {
                core::ptr::write_bytes(buffer.as_ptr(), 0, buffer.len());
                let slice = core::slice::from_raw_parts_mut(buffer.as_ptr(), buffer.len());
                io::IoSliceMut::new(slice)
            }).collect();
            <T as io::Read>::read_vectored(self, &mut buffers)
        }
    }
}

impl<T: io::Write> super::Write for T {
    type Error = io::Error;
    #[inline]
    fn flush(&mut self) -> Result<(), Self::Error> {
        <T as io::Write>::flush(self)
    }
    #[inline]
    fn write(&mut self, slice: &[u8]) -> Result<usize, Self::Error> {
        <T as io::Write>::write(self, slice)
    }
}
