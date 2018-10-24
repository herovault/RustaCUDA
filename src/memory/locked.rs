use super::DeviceCopy;
use error::*;
use memory::{cuda_free_locked, cuda_malloc_locked};
use std::mem;
use std::ops;
use std::ptr;
use std::slice;

/// Fixed-size host-side buffer in page-locked memory. See the
/// [`module-level documentation`](../memory/index.html) for more details on page-locked memory.
#[derive(Debug)]
pub struct LockedBuffer<T: DeviceCopy> {
    buf: *mut T,
    capacity: usize,
}
impl<T: DeviceCopy> LockedBuffer<T> {
    /// Allocate a new page-locked buffer large enough to hold `size` `T`'s and initialized with
    /// clones of `value`.
    ///
    /// # Errors:
    ///
    /// If the allocation fails, returns the error from CUDA. If `size` is large enough that
    /// `size * mem::sizeof::<T>()` overflows usize, then returns InvalidMemoryAllocation.
    ///
    /// # Examples:
    ///
    /// ```
    /// use rustacuda::memory::*;
    /// let mut buffer = LockedBuffer::new(&0u64, 5).unwrap();
    /// buffer[0] = 1;
    /// ```
    pub fn new(value: &T, size: usize) -> CudaResult<Self> {
        unsafe {
            let mut uninit = LockedBuffer::uninitialized(size)?;
            for x in 0..size {
                *uninit.get_unchecked_mut(x) = value.clone();
            }
            Ok(uninit)
        }
    }

    /// Allocate a new page-locked buffer of the same size as `slice`, initialized with a clone of
    /// the data in `slice`.
    ///
    /// # Errors:
    ///
    /// If the allocation fails, returns the error from CUDA.
    ///
    /// # Examples:
    ///
    /// ```
    /// use rustacuda::memory::*;
    /// let values = [0u64; 5];
    /// let mut buffer = LockedBuffer::from_slice(&values).unwrap();
    /// buffer[0] = 1;
    /// ```
    pub fn from_slice(slice: &[T]) -> CudaResult<Self> {
        unsafe {
            let mut uninit = LockedBuffer::uninitialized(slice.len())?;
            for (i, x) in slice.iter().enumerate() {
                *uninit.get_unchecked_mut(i) = x.clone();
            }
            Ok(uninit)
        }
    }

    /// Allocate a new page-locked buffer large enough to hold `size` `T`'s, but without
    /// initializing the contents.
    ///
    /// # Errors:
    ///
    /// If the allocation fails, returns the error from CUDA. If `size` is large enough that
    /// `size * mem::sizeof::<T>()` overflows usize, then returns InvalidMemoryAllocation.
    ///
    /// # Safety:
    ///
    /// The caller must ensure that the contents of the buffer are initialized before reading from
    /// the buffer.
    ///
    /// # Examples:
    ///
    /// ```
    /// use rustacuda::memory::*;
    /// let mut buffer = unsafe { LockedBuffer::uninitialized(5).unwrap() };
    /// for i in buffer.iter_mut() {
    ///     *i = 0u64;
    /// }
    /// ```
    pub unsafe fn uninitialized(size: usize) -> CudaResult<Self> {
        let bytes = size.checked_mul(mem::size_of::<T>())
            .ok_or(CudaError::InvalidMemoryAllocation)?;

        let ptr: *mut T = if bytes > 0 {
            cuda_malloc_locked(bytes)?
        } else {
            ptr::NonNull::dangling().as_ptr()
        };
        Ok(LockedBuffer {
            buf: ptr as *mut T,
            capacity: size,
        })
    }

    /// Extracts a slice containing the entire buffer.
    ///
    /// Equivalent to `&s[..]`.
    ///
    /// # Examples:
    ///
    /// ```
    /// use rustacuda::memory::*;
    /// let buffer = LockedBuffer::new(&0u64, 5).unwrap();
    /// let sum : u64 = buffer.as_slice().iter().sum();
    /// ```
    pub fn as_slice(&self) -> &[T] {
        self
    }

    /// Extracts a mutable slice of the entire vector.
    ///
    /// Equivalent to `&mut s[..]`.
    ///
    /// # Examples:
    ///
    /// ```
    /// use rustacuda::memory::*;
    /// let mut buffer = LockedBuffer::new(&0u64, 5).unwrap();
    /// for i in buffer.as_mut_slice() {
    ///     *i = 12u64;
    /// }
    /// ```
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self
    }

    /// Creates a `LockedBuffer<T>` directly from the raw components of another locked buffer.
    ///
    /// # Safety
    ///
    /// This is highly unsafe, due to the number of invariants that aren't
    /// checked:
    ///
    /// * `ptr` needs to have been previously allocated via `LockedBuffer` or
    /// [`cuda_malloc_locked`](fn.cuda_malloc_locked.html).
    /// * `ptr`'s `T` needs to have the same size and alignment as it was allocated with.
    /// * `capacity` needs to be the capacity that the pointer was allocated with.
    ///
    /// Violating these may cause problems like corrupting the CUDA driver's
    /// internal data structures.
    ///
    /// The ownership of `ptr` is effectively transferred to the
    /// `LockedBuffer<T>` which may then deallocate, reallocate or change the
    /// contents of memory pointed to by the pointer at will. Ensure
    /// that nothing else uses the pointer after calling this
    /// function.
    ///
    /// # Examples:
    ///
    /// ```
    /// use std::mem;
    /// use rustacuda::memory::*;
    ///
    /// let mut buffer = LockedBuffer::new(&0u64, 5).unwrap();
    /// let ptr = buffer.as_mut_ptr();
    /// let size = buffer.len();
    ///
    /// mem::forget(buffer);
    ///
    /// let buffer = unsafe { LockedBuffer::from_raw_parts(ptr, size) };
    /// ```
    pub unsafe fn from_raw_parts(ptr: *mut T, size: usize) -> LockedBuffer<T> {
        LockedBuffer {
            buf: ptr,
            capacity: size,
        }
    }
}

impl<T: DeviceCopy> AsRef<[T]> for LockedBuffer<T> {
    fn as_ref(&self) -> &[T] {
        self
    }
}
impl<T: DeviceCopy> AsMut<[T]> for LockedBuffer<T> {
    fn as_mut(&mut self) -> &mut [T] {
        self
    }
}
impl<T: DeviceCopy> ops::Deref for LockedBuffer<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe {
            let p = self.buf;
            slice::from_raw_parts(p, self.capacity)
        }
    }
}
impl<T: DeviceCopy> ops::DerefMut for LockedBuffer<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            let ptr = self.buf;
            slice::from_raw_parts_mut(ptr, self.capacity)
        }
    }
}
impl<T: DeviceCopy> Drop for LockedBuffer<T> {
    fn drop(&mut self) {
        if self.capacity > 0 && mem::size_of::<T>() > 0 {
            // No choice but to panic if this fails.
            unsafe {
                cuda_free_locked(self.buf).expect("Failed to deallocate CUDA page-locked memory.");
            }
        }
        self.capacity = 0;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::mem;

    #[derive(Clone, Debug)]
    struct ZeroSizedType;
    unsafe impl ::memory::DeviceCopy for ZeroSizedType {}

    #[test]
    fn test_new() {
        let val = 0u64;
        let mut buffer = LockedBuffer::new(&val, 5).unwrap();
        buffer[0] = 1;
    }

    #[test]
    fn test_from_slice() {
        let values = [0u64; 10];
        let mut buffer = LockedBuffer::from_slice(&values).unwrap();
        for i in buffer[0..3].iter_mut() {
            *i = 10;
        }
    }

    #[test]
    fn from_raw_parts() {
        let mut buffer = LockedBuffer::new(&0u64, 5).unwrap();
        buffer[2] = 1;
        let ptr = buffer.as_mut_ptr();
        let len = buffer.len();
        mem::forget(buffer);

        let buffer = unsafe { LockedBuffer::from_raw_parts(ptr, len) };
        assert_eq!(&[0u64, 0, 1, 0, 0], buffer.as_slice());
        drop(buffer);
    }

    #[test]
    fn zero_length_buffer() {
        let buffer = LockedBuffer::new(&0u64, 0).unwrap();
        drop(buffer);
    }

    #[test]
    fn zero_size_type() {
        let buffer = LockedBuffer::new(&ZeroSizedType, 10).unwrap();
        drop(buffer);
    }

    #[test]
    fn overflows_usize() {
        let err = LockedBuffer::new(&0u64, ::std::usize::MAX - 1).unwrap_err();
        assert_eq!(CudaError::InvalidMemoryAllocation, err);
    }
}