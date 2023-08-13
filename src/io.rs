use crate::RingBuffer;

// It would be good to use raw slices instead of raw pointer and length pairs.
// Blocking: https://github.com/rust-lang/rust/issues/74265

pub trait Read {
    type Error;
    /// Place the next bytes from the reader in to the `buffer` and returns the
    /// number of bytes written, and therefore initialized.
    fn read(&mut self, buffer: &mut [MaybeUninit<u8>]) -> Result<usize, Self::Error>;
    /// Read bytes in to the regions specified by the [`IoVecMut`] entries.
    /// Returns the number of bytes read in to the buffer, and therefore initialized.
    fn read_vectored(&mut self, buffers: &mut [IoVecMut]) -> Result<usize, Self::Error> {
        let mut read = 0;
        for buffer in buffers {
            read += self.read(buffer.as_maybe_uninit_slice())?;
        }
        Ok(read)
    }
}

pub trait Write {
    type Error;
    /// Write `slice` to this writer.
    /// Returns the number of bytes that were written.
    fn write(&mut self, slice: &[u8]) -> usize;
    /// Ensure written bytes are visible to other readers of the resource.
    fn flush(&mut self);
}

use core::{ptr::NonNull, marker::PhantomData, mem::MaybeUninit};
#[cfg(not(feature = "no_std"))]
use std::io;

pub struct BufStream<S: Sized + Read + Write> {
    stream: S,
    input: RingBuffer<u8>,
    output: RingBuffer<u8>,
}
impl<S: Sized + Read + Write> BufStream<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            input: RingBuffer::new(),
            output: RingBuffer::new(),
        }
    }
    /// Read from the reader in to the internal buffer.
    pub fn buffer_read(&mut self) -> Result<(), <S as Read>::Error> {
        let (lhs, rhs) = self.input.spare_capacity_mut();
        let count = self.stream.read_vectored(&mut [
            lhs.into(),
            rhs.into()
        ])?;
        // Safety: The count is no larger than the space available from `spare_capacity_mut`.
        unsafe {
            self.input.set_write_cursor(count)
        };
        Ok(())
    }
}
impl<S: Sized + Read + Write> Read for BufStream<S> {
    type Error = <S as Read>::Error;
    fn read(&mut self, buffer: &mut [MaybeUninit<u8>]) -> Result<usize, Self::Error> {
        self.buffer_read()?;
        todo!()
    }
    fn read_vectored(&mut self, buffers: &mut [IoVecMut]) -> Result<usize, Self::Error> {
        self.buffer_read()?;
        todo!()
    }
}

pub struct BufReader<> {

}
pub struct BufWriter<> {

}

/// An immutable slice used for vectored IO.
/// 
/// On Linux this is ABI compatible with struct iovec.
#[repr(C)]
pub struct IoVec<'a> {
    ptr: *const u8,
    len: usize,
    _marker: PhantomData<&'a [u8]>
}
impl<'a> IoVec<'a> {
    #[inline]
    pub fn new(slice: &'a [u8]) -> Self {
        Self {
            ptr: slice.as_ptr(),
            len: slice.len(),
            _marker: PhantomData,
        }
    }
    /// Construct a new `IoVec` that may point to uninitialised data.
    #[inline]
    pub fn maybe_uninit(slice: &'a [MaybeUninit<u8>]) -> Self {
        Self {
            ptr: slice.as_ptr() as *const u8,
            len: slice.len(),
            _marker: PhantomData
        }
    }
    /// Construct a new `IoVec` from a pointer and length that may point to
    /// uninitialised data.
    /// # Safety
    /// The requirements of [`core::slice::from_raw_parts`] must be met.
    /// Extra care must be taken with the lifetime.
    #[inline]
    pub unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> Self {
        Self {
            ptr,
            len,
            _marker: PhantomData
        }
    }
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub fn as_maybe_uninit_slice(&self) -> &'a [MaybeUninit<u8>] {
        // Safety: The requirements of a slice are required to make a `IoVec`.
        unsafe {
            core::slice::from_raw_parts(
                self.ptr as *const MaybeUninit<u8>,
                self.len
            )
        }
    }
}
impl<'a> From<&'a [u8]> for IoVec<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self::new(value)
    }
}
impl<'a> From<&'a [MaybeUninit<u8>]> for IoVec<'a> {
    fn from(value: &'a [MaybeUninit<u8>]) -> Self {
        Self::maybe_uninit(value)
    }
}
impl<'a> From<&IoVec<'a>> for &'a [MaybeUninit<u8>] {
    fn from(value: &IoVec<'a>) -> Self {
        value.as_maybe_uninit_slice()
    }
}

/// A mutable slice used for vectored IO.
/// 
/// On Linux this is ABI compatible with struct iovec.
#[repr(C)]
pub struct IoVecMut<'a> {
    ptr: *mut u8,
    len: usize,
    _marker: PhantomData<&'a mut [u8]>
}
impl<'a> IoVecMut<'a> {
    #[inline]
    pub fn new(slice: &'a mut [u8]) -> Self {
        Self {
            ptr: slice.as_mut_ptr(),
            len: slice.len(),
            _marker: PhantomData,
        }
    }
    /// Construct a new `IoVecMut` that may point to uninitialised data.
    #[inline]
    pub fn maybe_uninit(slice: &'a mut [MaybeUninit<u8>]) -> Self {
        Self {
            ptr: slice.as_ptr() as *mut u8,
            len: slice.len(),
            _marker: PhantomData
        }
    }
    /// Construct a new `IoVecMut` from a pointer and length that may point to
    /// uninitialised data.
    /// # Safety
    /// The requirements of [`core::slice::from_raw_parts_mut`] must be met.
    /// Extra care must be taken with the lifetime.
    #[inline]
    pub unsafe fn from_raw_parts(ptr: *mut u8, len: usize) -> Self {
        Self {
            ptr,
            len,
            _marker: PhantomData
        }
    }
    #[inline]
    pub fn as_ptr(&self) -> NonNull<u8> {
        unsafe {
            NonNull::new_unchecked(self.ptr)
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub fn as_maybe_uninit_slice(&mut self) -> &'a mut [MaybeUninit<u8>] {
        // Safety: The requirements of a slice are required to make a `IoVecMut`.
        unsafe {
            core::slice::from_raw_parts_mut(
                self.ptr as *mut MaybeUninit<u8>,
                self.len
            )
        }
    }
}
impl<'a> From<&'a mut [u8]> for IoVecMut<'a> {
    fn from(value: &'a mut [u8]) -> Self {
        Self::new(value)
    }
}
impl<'a> From<&'a mut [MaybeUninit<u8>]> for IoVecMut<'a> {
    fn from(value: &'a mut [MaybeUninit<u8>]) -> Self {
        Self::maybe_uninit(value)
    }
}
impl<'a> From<&mut IoVecMut<'a>> for &'a mut [MaybeUninit<u8>] {
    fn from(value: &mut IoVecMut<'a>) -> Self {
        value.as_maybe_uninit_slice()
    }
}
