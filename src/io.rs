use crate::RingBuffer;

#[cfg(not(feature = "no_std"))]
pub mod std;

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
    fn write(&mut self, slice: &[u8]) -> Result<usize, Self::Error>;
    /// Ensure written bytes are visible to other readers of the resource.
    fn flush(&mut self) -> Result<(), Self::Error>;
}

use core::{marker::PhantomData, mem::MaybeUninit};

pub struct BufStream<S: Sized + Read + Write> {
    stream: S,
    input: RingBuffer<u8>,
    output: RingBuffer<u8>,
}
impl<S: Sized + Read + Write> BufStream<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            input: RingBuffer::with_capacity(0),
            output: RingBuffer::with_capacity(0),
        }
    }
    /// Create a new buffered stream with a capacity of at least `capcity` bytes
    /// for both input and output buffers.
    pub fn with_capacity(stream: S, capacity: usize) -> Self {
        Self {
            stream,
            input: RingBuffer::with_capacity(capacity),
            output: RingBuffer::with_capacity(capacity),
        }
    }
    /// Read from the reader in to the internal buffer.
    pub fn buffer_read(&mut self) -> Result<(), <S as Read>::Error> {
        if self.input.full() {
            self.input.reserve(1);
        }
        let (lhs, rhs) = self.input.spare_capacity_mut();
        let parts = match (lhs.len(), rhs.len()) {
            (0, 0) => 0,
            (_, 0) => 1,
            (_, _) => 2,
        };
        let count = self.stream.read_vectored(&mut [
            lhs.into(),
            rhs.into()
        ][..parts])?;
        // Safety: The count is no larger than the space available from `spare_capacity_mut`.
        unsafe {
            self.input.set_write_cursor(count)
        };
        // TODO: a smarter growth strategy
        if self.input.full() {
            self.input.reserve(1);
        }
        Ok(())
    }

    fn read_into(&mut self, buffer: &mut [MaybeUninit<u8>]) -> Result<usize, <S as Read>::Error> {
        let (lhs, rhs) = self.input.as_mut_slices();
        let ptr = buffer.as_mut_ptr() as *mut u8;
        let lhs_len = buffer.len().min(lhs.len());
        let rhs_len = (buffer.len() - lhs_len).min(rhs.len());
        let total_len = lhs_len + rhs_len;
        // Safety:
        // - `buffer` is valid for at least `lhs_len + rhs_len` writes.
        // - `lhs` is valid for at least `lhs_len` reads.
        // - `rhs` is valid for at least `rhs_len` reads.
        // - Therefore the input buffer is valid for at least `total_len` reads.
        // - `lhs`, `rhs` and `buffer` are mutable slice and therefore must be aligned and non-aliasing.
        unsafe {
            ptr.copy_from_nonoverlapping(lhs.as_ptr(), lhs_len);
            ptr.offset(lhs_len as isize).copy_from_nonoverlapping(rhs.as_ptr(), rhs_len);
            self.input.set_read_cursor(total_len);
        }
        Ok(total_len)
    }
}
impl<S: Sized + Read + Write> Read for BufStream<S> {
    type Error = <S as Read>::Error;
    fn read(&mut self, buffer: &mut [MaybeUninit<u8>]) -> Result<usize, Self::Error> {
        // TODO: avoid buffering when provided with a large enough buffer anyway.
        self.buffer_read()?;
        self.read_into(buffer)
    }
    fn read_vectored(&mut self, buffers: &mut [IoVecMut]) -> Result<usize, Self::Error> {
        self.buffer_read()?;
        let mut read = 0;
        for buffer in buffers {
            read += self.read_into(buffer.as_maybe_uninit_slice())?;
        }
        Ok(read)
    }
}
// impl<S: Sized + Read + Write> Write for BufStream<S> {
//     type Error = <S as Write>::Error;
//     fn flush(&mut self) -> Result<(), Self::Error> {
//         self.stream.flush()
//     }
//     fn write(&mut self, slice: &[u8]) -> Result<usize, Self::Error> {
//         
//     }
// }

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
            ptr: slice.as_mut_ptr() as *mut u8,
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
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
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
