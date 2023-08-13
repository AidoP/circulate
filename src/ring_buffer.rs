
extern crate alloc;
use alloc::alloc::{alloc, dealloc};
use core::{alloc::Layout, marker::PhantomData, mem::{size_of, align_of, MaybeUninit}, ptr::{NonNull, drop_in_place}};

/// A heap-allocated circular buffer.
/// ```rust
/// use circulate::RingBuffer;
/// let mut fruit = RingBuffer::new();
/// fruit.push("apples");
/// fruit.push("oranges");
/// fruit.push("pears");
/// fruit.push("grapes");
/// for f in fruit {
///     println!("Yummy {f}!")
/// }
/// ```
pub struct RingBuffer<T> {
    data: NonNull<T>,
    // TODO: It may be better to store the mask (ie. capacity - 1) rather than the capacity.
    capacity: usize,
    /// The index of the element to read next.
    read: usize,
    /// The index of the element to write next.
    write: usize,
    _phantom: PhantomData<T>,
}
impl<T> RingBuffer<T> {
    pub const fn new() -> Self {
        Self {
            data: NonNull::dangling(),
            capacity: 0,
            read: 0,
            write: 0,
            _phantom: PhantomData,
        }
    }
    fn alloc(capacity: usize) -> (NonNull<T>, usize) {
        if let Some(layout) = Self::layout_for(capacity) {
            // Safety: layout is non-zero.
            let ptr = unsafe { alloc(layout) };
            if ptr.is_null() {
                alloc::alloc::handle_alloc_error(layout);
            }
            unsafe { (NonNull::new_unchecked(ptr).cast(), layout.size() / size_of::<T>()) }
        } else {
            (NonNull::dangling(), 0)
        }
    }
    /// Create a new [`RingBuffer`] with space for at least `capacity` elements.
    pub fn with_capacity(capacity: usize) -> Self {
        let (data, capacity) = Self::alloc(capacity);
        Self {
            data,
            capacity,
            read: 0,
            write: 0,
            _phantom: PhantomData,
        }
    }

    /// Ensure there is space for at least `count` more elements.
    pub fn reserve(&mut self, count: usize) {
        let Some(layout) = Self::layout_for(self.capacity + count) else {
            // Reserved 0 bytes while empty.
            return;
        };
        // Note: If `realloc()` is used the data may need an extra move, it may be more efficient to
        // just use `alloc()` and `dealloc()` so only the necessary data is copied.
        // Safety: layout is non-zero.
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            alloc::alloc::handle_alloc_error(layout);
        }
        let data = unsafe { NonNull::new_unchecked(ptr).cast() };
        let capacity = layout.size() / size_of::<T>();

        let Some(old_layout) = self.layout() else {
            // No previous allocation
            self.data = data.cast();
            self.capacity = capacity;
            return;
        };

        {
            let (data_lhs, data_rhs) = self.as_mut_slices();
            // Safety: The new `data` pointer points to a larger area than the old data.
            unsafe {
                let lhs_bytes =  data_lhs.len() * size_of::<T>();
                <*mut u8>::copy_from(data.as_ptr(), data_lhs.as_ptr().cast(), lhs_bytes);
                <*mut u8>::copy_from(data.as_ptr().offset(lhs_bytes as isize), data_rhs.as_ptr().cast(), data_rhs.len() * size_of::<T>());
            }
        }

        unsafe {
            dealloc(self.data.as_ptr().cast(), old_layout);
        }
        self.data = data.cast();
        self.capacity = capacity;
    }

    /// Remove all values from the [`RingBuffer`].
    /// The previous capacity will be retained.
    pub fn clear(&mut self) {
        let (left, right) = self.as_mut_slices();
        // Safety: Slices have the same requirements as `drop_in_place()`.
        unsafe {
            drop_in_place(left);
            drop_in_place(right);
        }
        self.read = 0;
        self.write = 0;
    }

    /// Returns if there are no items in the buffer.
    pub const fn empty(&self) -> bool {
        self.read == self.write
    }
    /// Returns if the length of the buffer has reached its capacity.
    pub const fn full(&self) -> bool {
        (self.write + 1) & self.mask() == self.read
    }
    
    /// Get the number of items in the [`RingBuffer`].
    pub const fn len(&self) -> usize {
        if self.read <= self.write {
            self.write - self.read
        } else {
            self.capacity - (self.read - self.write)
        }
    }

    /// Set the read cursor to point to `count` items past the current location.
    /// # Safety
    /// The buffer must be readable for `count` more elements.
    /// The `count` must not overflow one less than the remaining `capacity`,
    /// an equal read and write cursor indicates an empty [`RingBuffer`].
    pub unsafe fn set_read_cursor(&mut self, count: usize) {
        self.read = (self.read + count) & self.mask();
    }
    /// Set the write cursor to point to `count` items past the current location.
    /// # Safety
    /// The buffer must be writable for `count` more elements.
    /// The `count` must not overflow one less than the remaining `capacity`,
    /// an equal read and write cursor indicates an empty [`RingBuffer`].
    pub unsafe fn set_write_cursor(&mut self, count: usize) {
        self.write = (self.write + count) & self.mask();
    }

    #[inline(always)]
    const fn mask(&self) -> usize {
        self.capacity.saturating_sub(1)
    }
    /// Index the read pointer by `index` items.
    /// The returned pointer my not be valid for reads if index does
    /// not refer to an initialized element.
    #[inline]
    const fn read_ptr(&self, index: usize) -> *mut T {
        // Safety:
        // - Capacity is guaranteed to be smaller than `isize::MAX`.
        // - Masking by capacity ensures the computed offset is in range.
        unsafe {
            self.data.as_ptr().offset(((self.read + index) & self.mask()) as isize)
        }
    }
    
    pub fn get(&self, index: usize) -> Option<&T> {
        if self.empty() {
            None
        } else {
            // Safety: `read` must be pointing at an initialized element.
            unsafe {
                Some(&*self.read_ptr(index))
            }
        }
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if self.empty() {
            None
        } else {
            // Safety: `read` must be pointing at an initialized element.
            unsafe {
                Some(&mut *self.read_ptr(index))
            }
        }
    }

    /// Returns an iterator over the values in the buffer.
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            data: self.data,
            mask: self.mask(),
            len: self.len(),
            cursor: self.read,
            _marker: PhantomData
        }
    }
    /// Returns an iterator that allows mutating the values in the buffer.
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            data: self.data,
            mask: self.mask(),
            len: self.len(),
            cursor: self.read,
            _marker: PhantomData
        }
    }

    /// Push an item to the write end of the [`RingBuffer`].
    pub fn push(&mut self, value: T) {
        if self.full() {
            self.reserve(1)
        }
        // Safety: Space was reserved for at least one more write and write is always a valid offset.
        unsafe {
            self.data.as_ptr().offset(self.write as isize).write(value);
        }
        self.write = (self.write + 1) & self.mask();
    }
    /// Take the next item from the read end of the [`RingBuffer`], or return [`None`] if the buffer is empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.empty() {
            return None;
        }

        let read = self.read;
        self.read = (self.read + 1) & self.mask();
        
        // Safety: The capacity will not exceed `isize::MAX` so `read` is a valid offset.
        unsafe {
            Some(self.data.as_ptr().offset(read as isize).read())
        }
    }

    pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        if self.read < self.write {
            unsafe {(
                core::slice::from_raw_parts_mut(self.data.as_ptr().offset(self.read as isize), self.write - self.read),
                &mut []
            )}
        } else {
            unsafe {(
                core::slice::from_raw_parts_mut(self.data.as_ptr().offset(self.read as isize), self.capacity - self.read),
                core::slice::from_raw_parts_mut(self.data.as_ptr(), self.write)
            )}
        }
    }
    /// Get slices over the uninitialized items.
    pub fn spare_capacity_mut(&mut self) -> (&mut [MaybeUninit<T>], &mut [MaybeUninit<T>]) {
        if self.read < self.write {
            // Safety: It is guaranteed that the offsets cannot overflow an isize.
            unsafe {
                (
                    core::slice::from_raw_parts_mut(
                        self.data.as_ptr().offset(self.write as isize) as *mut MaybeUninit<T>,
                        self.capacity - self.write
                    ),
                    core::slice::from_raw_parts_mut(
                        self.data.as_ptr() as *mut MaybeUninit<T>,
                        self.read.saturating_sub(1)
                    )
                )
            }
        } else {
            // Safety: It is guaranteed that the offsets cannot overflow an isize.
            unsafe {
                (
                    core::slice::from_raw_parts_mut(
                        self.data.as_ptr().offset(self.write as isize) as *mut MaybeUninit<T>,
                        (self.read - self.write).saturating_sub(1)
                    ),
                    &mut []
                )
            }
        }
    }

    pub const fn layout(&self) -> Option<Layout> {
        if size_of::<T>() == 0 || self.capacity == 0 {
            None
        } else {
            assert!(size_of::<T>() % align_of::<T>() == 0);
            // Safety:
            // - Rust types are asserted to have a matching size and stride.
            // - `align_of` will always return a power of 2.
            // - The capacity in bytes will never overflow an isize.
            unsafe {
                let align = align_of::<T>();
                let size = size_of::<T>() * self.capacity;
                let layout = Layout::from_size_align_unchecked(size, align);
                Some(layout)
            }
        }
    }
    /// Get a layout valid for the ring buffer with a size of at least `capacity` items.
    /// It provides the following guarantees:
    /// - The layout will allow indexing by `data.offset()`.
    /// - The layout size in items will be a power of two.
    /// To guard against misuse, [`None`] is returned if the layout would have a 0 size.
    fn layout_for(capacity: usize) -> Option<Layout> {
        let capacity = capacity.next_power_of_two();

        const fn max_size_for_align(align: usize) -> usize {
            isize::MAX as usize - (align - 1)
        }
        
        if size_of::<T>() == 0 || capacity == 0 {
            return None;
        }
        if capacity > max_size_for_align(align_of::<T>()) / size_of::<T>() {
            capacity_overflow()
        }

        // Note: A capacity of 1 is an effective capacity of 0.
        // Safety:
        // - `size` > 0.
        // - `capacity` does not overflow `isize::MAX`.
        let size = capacity.max(2) * size_of::<T>();
        unsafe {
            Some(Layout::from_size_align_unchecked(size, align_of::<T>()))
        }
    }
}
impl<T> Drop for RingBuffer<T> {
    fn drop(&mut self) {
        let (left, right) = self.as_mut_slices();
        // Safety: Slices have the same requirements as `drop_in_place()`.
        unsafe {
            drop_in_place(left);
            drop_in_place(right);
        }

        if let Some(layout) = self.layout() {
            // Safety:
            // - The pointer must point to owned memory of the layout if `layout()` returns `Some`.
            unsafe {
                dealloc(self.data.as_ptr().cast(), layout)
            }
        }
    }
}

impl<T> IntoIterator for RingBuffer<T> {
    type IntoIter = IntoIter<T>;
    type Item = T;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self)
    }
}

pub struct IntoIter<T>(RingBuffer<T>);
impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop()
    }
    fn count(self) -> usize {
        self.0.len()
    }
}

pub struct Iter<'a, T> {
    data: NonNull<T>,
    mask: usize,
    len: usize,
    cursor: usize,
    _marker: PhantomData<&'a T>,
}
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            // Safety:
            // - Capacity is guaranteed to be smaller than `isize::MAX`.
            // - The cursor is masked by capacity ensuring the computed offset is in range.
            // - The cursor is in range of the initialized `len`.
            unsafe {
                let ptr = self.data.as_ptr().offset(self.cursor as isize);
                self.len -= 1;
                self.cursor = (self.cursor + 1) & self.mask;
                Some(&*ptr)
            }
        }
    }
}

pub struct IterMut<'a, T> {
    data: NonNull<T>,
    mask: usize,
    len: usize,
    cursor: usize,
    _marker: PhantomData<&'a T>,
}
impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            // Safety:
            // - Capacity is guaranteed to be smaller than `isize::MAX`.
            // - The cursor is masked by capacity ensuring the computed offset is in range.
            // - The cursor is in range of the initialized `len`.
            unsafe {
                let ptr = self.data.as_ptr().offset(self.cursor as isize);
                self.len -= 1;
                self.cursor = (self.cursor + 1) & self.mask;
                Some(&mut *ptr)
            }
        }
    }
}

const fn capacity_overflow() -> ! {
    panic!("capacity overflow")
}
