use std::alloc::{alloc, alloc_zeroed, dealloc, realloc, Layout};
use std::collections::HashMap;
use std::ffi::{c_int, c_void};
use std::ptr::{null, null_mut};

use bindings::{
    fsm, fsm_alloc, fsm_determinise, fsm_free, fsm_print_lang_FSM_PRINT_RUST, re_comp,
    re_dialect_RE_PCRE, re_err, re_flags_RE_END_NL, re_strerror,
};

pub mod bindings;

type AllocMap = HashMap<*mut u8, Layout>;

#[derive(Debug)]
pub struct FsmAlloc {
    a: std::pin::Pin<Box<fsm_alloc>>,
}

impl FsmAlloc {
    fn new() -> Self {
        macro_rules! get_allocs {
            ($opaque:expr) => {
                ($opaque as *mut AllocMap).as_mut().expect("opaque is null")
            };
        }

        unsafe extern "C" fn fsm_free(opaque: *mut c_void, p: *mut c_void) {
            let p = p as *mut u8;
            if p.is_null() {
                return;
            }

            let a = unsafe { get_allocs!(opaque) };
            let Some(l) = a.remove(&p) else {
                return;
            };

            dealloc(p, l)
        }
        unsafe extern "C" fn fsm_calloc(opaque: *mut c_void, n: usize, sz: usize) -> *mut c_void {
            let Some(align) = sz.checked_next_power_of_two() else {
                return null_mut();
            };
            let Some(size) = n.checked_mul(sz) else {
                return null_mut();
            };
            let Ok(l) = Layout::from_size_align(size, align) else {
                return null_mut();
            };

            let p = alloc_zeroed(l);
            let a = unsafe { get_allocs!(opaque) };
            if let Some(existing) = a.insert(p as *mut _, l) {
                panic!("calloced an existing pointer: {p:?} with layout {existing:?}");
            }

            p as *mut _
        }
        unsafe extern "C" fn fsm_malloc(opaque: *mut c_void, sz: usize) -> *mut c_void {
            let Some(align) = sz.checked_next_power_of_two() else {
                return null_mut();
            };
            let Ok(l) = Layout::from_size_align(sz, align) else {
                return null_mut();
            };

            let p = alloc(l);
            let a = unsafe { get_allocs!(opaque) };
            if let Some(existing) = a.insert(p as *mut _, l) {
                panic!("malloced an existing pointer: {p:?} with layout {existing:?}");
            }

            p as *mut _
        }
        unsafe extern "C" fn fsm_realloc(
            opaque: *mut c_void,
            p: *mut c_void,
            sz: usize,
        ) -> *mut c_void {
            let p = p as *mut u8;
            if p.is_null() {
                return fsm_malloc(opaque, sz);
            }

            let a = unsafe { get_allocs!(opaque) };
            let Some(removed) = a.remove(&p) else {
                return null_mut();
            };
            let Ok(l) = Layout::from_size_align(sz, removed.align()) else {
                return null_mut();
            };
            let p = realloc(p, removed, sz);
            if let Some(existing) = a.insert(p as *mut _, l) {
                panic!("malloced an existing pointer: {p:?} with layout {existing:?}");
            }

            p as *mut _
        }

        let allocations: AllocMap = AllocMap::new();
        let allocations = Box::new(allocations);

        let a = fsm_alloc {
            free: Some(fsm_free),
            calloc: Some(fsm_calloc),
            malloc: Some(fsm_malloc),
            realloc: Some(fsm_realloc),
            opaque: Box::into_raw(allocations) as *mut _,
        };

        Self {
            a: Box::into_pin(Box::new(a)),
        }
    }
}

impl Drop for FsmAlloc {
    fn drop(&mut self) {
        let ptr = self.a.opaque as *mut AllocMap;
        let _allocations = unsafe { Box::from_raw(ptr) };
    }
}

#[derive(Debug)]
pub struct Fsm {
    // drop order is important, so `fsm` must come before `_alloc`. this is
    // because `fsm` makes references into the `_alloc` structure, and we will
    // segfault if this is done incorrectly.
    fsm: *mut fsm,
    _alloc: FsmAlloc,
}

struct GetCharState<'i> {
    iter: &'i mut dyn Iterator<Item = u8>,
}

unsafe extern "C" fn get_char(state: *mut c_void) -> c_int {
    let Some(g) = (unsafe { (state as *mut GetCharState).as_mut() }) else {
        return libc::EOF;
    };
    let Some(b) = g.iter.next() else {
        return libc::EOF;
    };
    b as c_int
}

impl Drop for Fsm {
    fn drop(&mut self) {
        unsafe { fsm_free(self.fsm) }
    }
}

impl Fsm {
    pub fn compile_pcre(mut bytes: impl Iterator<Item = u8>) -> Result<Self, String> {
        let mut state = GetCharState { iter: &mut bytes };
        let state_ref = &mut state;

        let mut alloc = FsmAlloc::new();
        let a = (&mut *alloc.a) as *mut _;
        let mut err = re_err::default();
        let err_ref = &mut err;
        let fsm = unsafe {
            re_comp(
                re_dialect_RE_PCRE,
                Some(get_char),
                state_ref as *mut _ as *mut _,
                a,
                re_flags_RE_END_NL,
                err_ref as *mut _,
            )
        };

        if fsm.is_null() {
            let e = unsafe { std::ffi::CStr::from_ptr(re_strerror(err.e)) };
            Err(String::from_utf8(e.to_bytes().to_vec()).unwrap())
        } else {
            let ret = unsafe { fsm_determinise(fsm) };
            if ret == 0 {
                return Err("could not determinize DFA".into());
            }

            Ok(Self { fsm, _alloc: alloc })
        }
    }

    pub fn print(&mut self) -> Result<Vec<u8>, std::io::Error> {
        // libfsm emits generated code to a `FILE *`. On posix environments, we could use something like `fopencookie()` to create a compatible interface. On Windows, this doesn't exist, and I'm not parti

        // to make this work on windows and on linux, we use a tempfile to hold
        // the intermediate value, and then read it back out again. ideally, we
        // update the libfsm API to allow non-FILE ways of emitting the stream.
        unsafe {
            let tmp = libc::tmpfile();
            if tmp.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let ret = bindings::fsm_print(
                tmp as *mut _,
                self.fsm,
                null(),
                null(),
                fsm_print_lang_FSM_PRINT_RUST,
            );
            if ret != 0 {
                return Err(std::io::Error::last_os_error());
            }
            let pos = libc::ftell(tmp);
            if pos < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let written_length = pos as usize;

            libc::rewind(tmp);
            let mut buf = vec![0u8; written_length];
            let ret = libc::fread(buf.as_mut_ptr() as *mut _, 1, written_length, tmp);

            if ret == 0 && libc::ferror(tmp) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            buf.truncate(ret);

            libc::fclose(tmp);

            Ok(buf)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn does_not_crash() {
        assert!(Fsm::compile_pcre(b"hello world".iter().copied()).is_ok());
    }
}
