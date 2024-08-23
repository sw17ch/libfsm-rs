use std::{
    alloc::{alloc, dealloc, Layout},
    ptr::NonNull,
};

#[derive(Debug)]
pub struct AllocGuard {
    layout: Layout,
    guard_bytes: usize,
    data_size: usize,
    ptr: NonNull<u8>,
}

impl AllocGuard {
    pub fn as_ptr(&self) -> *mut u8 {
        unsafe { self.ptr.as_ptr().add(self.guard_bytes) }
    }

    pub fn malloc(guard_sz: usize, sz: usize) -> Option<Self> {
        let total_size = sz + (2 * guard_sz);
        let Some(align) = total_size.checked_next_power_of_two() else {
            return None;
        };
        let Ok(layout) = Layout::from_size_align(total_size, align) else {
            return None;
        };
        let p = unsafe { alloc(layout) };
        let p = NonNull::new(p)?;

        // set up the guard bytes
        let front_offset = 0;
        let back_offset = front_offset + guard_sz + sz;
        unsafe { std::ptr::write_bytes(p.as_ptr().add(front_offset), b'F', guard_sz) };
        unsafe { std::ptr::write_bytes(p.as_ptr().add(back_offset), b'B', guard_sz) };

        Some(Self {
            layout,
            guard_bytes: guard_sz,
            data_size: sz,
            ptr: p,
        })
    }

    pub fn calloc(guard_sz: usize, n: usize, sz: usize) -> Option<Self> {
        let Some(sz) = n.checked_mul(sz) else {
            return None;
        };

        let total_size = sz + (2 * guard_sz);
        let Some(align) = total_size.checked_next_power_of_two() else {
            return None;
        };
        let Ok(layout) = Layout::from_size_align(total_size, align) else {
            return None;
        };
        let p = unsafe { alloc(layout) };
        let p = NonNull::new(p)?;

        // set up the guard bytes
        let front_offset = 0;
        let data_offset = front_offset + guard_sz;
        let back_offset = data_offset + sz;
        unsafe { std::ptr::write_bytes(p.as_ptr().add(front_offset), b'F', guard_sz) };
        unsafe { std::ptr::write_bytes(p.as_ptr().add(data_offset), 0, sz) };
        unsafe { std::ptr::write_bytes(p.as_ptr().add(back_offset), b'B', guard_sz) };

        Some(Self {
            layout,
            guard_bytes: guard_sz,
            data_size: sz,
            ptr: p,
        })
    }

    pub fn free(self) {
        self.check();

        let Self {
            layout,
            guard_bytes,
            data_size,
            ptr,
        } = self;

        let ptr = ptr.as_ptr();
        unsafe { std::ptr::write_bytes(ptr, b'X', data_size + (guard_bytes * 2)) };
        unsafe { dealloc(ptr, layout) }
    }

    pub fn check(&self) {
        let Self {
            layout: _,
            guard_bytes,
            data_size,
            ptr,
        } = self;

        let ptr = ptr.as_ptr();

        let front_offset = 0;
        let data_offset = front_offset + guard_bytes;
        let back_offset = data_offset + data_size;

        let front_slice =
            unsafe { std::slice::from_raw_parts(ptr.add(front_offset), *guard_bytes) };
        let back_slice = unsafe { std::slice::from_raw_parts(ptr.add(back_offset), *guard_bytes) };

        for b in front_slice {
            assert_eq!(b'F', *b);
        }
        for b in back_slice {
            assert_eq!(b'B', *b);
        }
    }

    pub fn data_size(&self) -> usize {
        self.data_size
    }
}
