use crate::RingBuffer;

// It would be good to use raw slices instead of raw pointer and length pairs.
// Blocking: https://github.com/rust-lang/rust/issues/74265

pub trait Read {
    unsafe fn read(&mut self, ptr: NonNull<u8>, len: usize);
    unsafe fn read_vectored(&mut self, ptr: NonNull<u8>, len: usize);
}

pub trait Write {
    unsafe fn write(&mut self, slice: &[u8]);
}

use core::{ptr::NonNull, marker::PhantomData};
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
    /// Construct a new `IoVecMut` that may point to uninitialised data.
    /// # Safety
    /// `ptr` must be writable for `len` bytes.
    #[inline]
    pub unsafe fn maybe_uninit(ptr: NonNull<u8>, len: usize) -> Self {
        Self {
            ptr: ptr.as_ptr(),
            len,
            _marker: core::marker::PhantomData
        }
    }
    #[inline]
    pub fn as_ptr(&mut self) -> *const u8 {
        self.ptr
    }
    #[inline]
    pub fn len(&mut self) -> usize {
        self.len
    }
}
impl<'a> From<&'a [u8]> for IoVec<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self::new(value)
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
    /// # Safety
    /// `ptr` must be writable for `len` bytes.
    #[inline]
    pub unsafe fn maybe_uninit(ptr: NonNull<u8>, len: usize) -> Self {
        Self {
            ptr: ptr.as_ptr(),
            len,
            _marker: core::marker::PhantomData
        }
    }
    #[inline]
    pub fn as_ptr(&mut self) -> NonNull<u8> {
        unsafe {
            NonNull::new_unchecked(self.ptr)
        }
    }
    #[inline]
    pub fn len(&mut self) -> usize {
        self.len
    }
}
impl<'a> From<&'a mut [u8]> for IoVecMut<'a> {
    fn from(value: &'a mut [u8]) -> Self {
        Self::new(value)
    }
}
