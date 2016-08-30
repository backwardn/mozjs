use glue;
use heapsize::HeapSizeOf;
use jsapi::root::*;
use rust::GCMethods;
use std::mem;
use std::ptr;

/// Types that can be traced.
///
/// This trait is unsafe; if it is implemented incorrectly, the GC may end up
/// collecting objects that are still reachable.
pub unsafe trait Trace {
    unsafe fn trace(&self, trc: *mut JSTracer);
}

/**
 * The Heap<T> class is a heap-stored reference to a JS GC thing. All members of
 * heap classes that refer to GC things should use Heap<T> (or possibly
 * TenuredHeap<T>, described below).
 *
 * Heap<T> is an abstraction that hides some of the complexity required to
 * maintain GC invariants for the contained reference. It uses operator
 * overloading to provide a normal pointer interface, but notifies the GC every
 * time the value it contains is updated. This is necessary for generational GC,
 * which keeps track of all pointers into the nursery.
 *
 * Heap<T> instances must be traced when their containing object is traced to
 * keep the pointed-to GC thing alive.
 *
 * Heap<T> objects should only be used on the heap. GC references stored on the
 * C/C++ stack must use Rooted/Handle/MutableHandle instead.
 *
 * Type T must be a public GC pointer type.
 */
#[repr(C)]
#[derive(Debug)]
pub struct Heap<T: GCMethods<T> + Copy> {
    ptr: T,
}

impl<T: GCMethods<T> + Copy> Heap<T> {
    pub fn new(v: T) -> Heap<T>
        where Heap<T>: Default
    {
        let mut ptr = Heap::default();
        ptr.set(v);
        ptr
    }

    pub fn set(&mut self, new_ptr: T) {
        unsafe {
            let prev = self.ptr;
            self.ptr = new_ptr;
            T::post_barrier(&mut self.ptr as _, prev, new_ptr);
        }
    }

    pub fn get(&self) -> T {
        self.ptr
    }

    pub unsafe fn get_unsafe(&self) -> *mut T {
        // TODO: We need to be able to (1) mark UnsafeCell as repr(C) somehow so
        // we can pass it across FFI boundaries, and then (2) instrument bindgen
        // to mark fields to be generated as UnsafeCell...
        mem::transmute(&self.ptr)
    }

    pub fn handle(&self) -> JS::Handle<T> {
        unsafe {
            JS::Handle::from_marked_location(&self.ptr as *const _)
        }
    }
}

impl<T: GCMethods<T> + Copy> Clone for Heap<T>
    where Heap<T>: Default
{
    fn clone(&self) -> Self {
        Heap::new(self.get())
    }
}

impl<T: GCMethods<T> + Copy + PartialEq> PartialEq for Heap<T> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl<T> Default for Heap<*mut T>
    where *mut T: GCMethods<*mut T> + Copy
{
    fn default() -> Heap<*mut T> {
        Heap {
            ptr: ptr::null_mut()
        }
    }
}

impl<T: GCMethods<T> + Copy> Drop for Heap<T> {
    fn drop(&mut self) {
        unsafe {
            let prev = self.ptr;
            T::post_barrier(&mut self.ptr as _, prev, T::initial());
        }
    }
}

// Creates a C string literal `$str`.
macro_rules! c_str {
    ($str:expr) => {
        concat!($str, "\0").as_ptr() as *const ::std::os::raw::c_char
    }
}

unsafe impl Trace for Heap<*mut JSFunction> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        glue::CallFunctionTracer(trc, self as *const _ as *mut Self, c_str!("function"));
    }
}

unsafe impl Trace for Heap<*mut JSObject> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        glue::CallObjectTracer(trc, self as *const _ as *mut Self, c_str!("object"));
    }
}

unsafe impl Trace for Heap<*mut JSScript> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        glue::CallScriptTracer(trc, self as *const _ as *mut Self, c_str!("script"));
    }
}

unsafe impl Trace for Heap<*mut JSString> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        glue::CallStringTracer(trc, self as *const _ as *mut Self, c_str!("string"));
    }
}

unsafe impl Trace for Heap<JS::Value> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        glue::CallValueTracer(trc, self as *const _ as *mut Self, c_str!("value"));
    }
}

unsafe impl Trace for Heap<jsid> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        glue::CallIdTracer(trc, self as *const _ as *mut Self, c_str!("id"));
    }
}

// This is measured properly by the heap measurement implemented in
// SpiderMonkey.
impl<T: Copy + GCMethods<T>> HeapSizeOf for Heap<T> {
    fn heap_size_of_children(&self) -> usize {
        0
    }
}