use core::pin::Pin;
use core::task::{Context, Poll};
use core::future::Future;
use core::ptr::NonNull;
use super::context::*;
use super::stack::DynamicStack;
use core::mem::transmute;
use std::alloc::Layout;
use std::ops::{Deref, DerefMut};


//stack frame
pub struct DynamicStackFrame{
    prev: Option<NonNull<Self>>, //not dropped with stack frame
    frame: Pin<Box<dyn StackContinue>>,
}
#[derive(Copy,Clone,Debug,PartialEq,Eq,Hash)]
#[repr(u8)]
pub enum FrameState{
    NewFrameCreated,
    AsyncYield,
    Ready,
}

pub struct DynamicStackFrameBox{
    value: Box<DynamicStackFrame>,
}
impl DynamicStackFrameBox{
    pub unsafe fn into_raw(self)->NonNull<DynamicStackFrame>{
        NonNull::new_unchecked(Box::into_raw(self.value))
    }
    pub unsafe fn from_raw(ptr: NonNull<DynamicStackFrame>)->Self{
        Self{value:Box::from_raw(ptr.as_ptr())}
    }
}
impl Deref for DynamicStackFrameBox{
    type Target = DynamicStackFrame;
    fn deref(&self) -> &Self::Target {self.value.deref()}
}
impl DerefMut for DynamicStackFrameBox{
    fn deref_mut(&mut self) -> &mut Self::Target {self.value.deref_mut()}
}

impl DynamicStackFrame{
    #[inline]
    unsafe fn make_unsafe_pin<'a,F>(fut: F)->Pin<Box<dyn StackContinue>> where F: Future + 'a{
        let dynamic: Pin<Box<dyn StackContinue + 'a>> = Box::pin(GenericStackFrame{
            result: Poll::Pending,
            future: fut,
        });
        //erase lifetimes
        transmute::<Pin<Box<dyn StackContinue + 'a>>,Pin<Box<dyn StackContinue>>>(dynamic)
    }
    #[inline]
    pub unsafe fn new<F>(fut: F)->DynamicStackFrameBox where F: Future{
        let frame = Self::make_unsafe_pin(fut);
        let value = Box::new(Self{frame,prev:None});
        DynamicStackFrameBox{value}
    }
    #[inline]
    pub fn is_ready(&self)->bool{!self.frame.is_pending()}
    #[inline]
    pub fn get_prev(&self)->Option<NonNull<Self>>{self.prev}
    #[inline]
    pub fn set_prev(&mut self,ptr: Option<NonNull<Self>>){self.prev = ptr;}
    ///take result and replace it with pending
    #[inline]
    pub unsafe fn take_result<T>(&mut self)->Poll<T>{
        let ptr = self.frame.as_mut().get_unchecked_mut().result_ptr() as *mut Poll<T>;
        ptr.replace(Poll::Pending)
    }
    pub fn stack_poll(&mut self,cx: &mut Context<'_>,stack: NonNull<DynamicStack>)->FrameState{
        unsafe{self.frame.as_mut().get_unchecked_mut().stack_poll(cx,stack)}
    }
}

trait StackContinue{
    /// if last poll returned pending or if frame was just created
    fn is_pending(&self)->bool;
    /// Allways points to last poll result, or pending if uninitialized. Will be dropped with this object.
    fn result_ptr(&mut self)->*mut ();
    /// Returs true if stack frame added next frame to stack
    /// and false otherwise.
    fn stack_poll(&mut self,cx: &mut Context<'_>,stack: NonNull<DynamicStack>)->FrameState;
}
//this struct is always pinned
struct GenericStackFrame<F: Future>{
    result: Poll<F::Output>,
    future: F,
}

impl<F: Future> StackContinue for GenericStackFrame<F>{
    fn is_pending(&self) -> bool { self.result.is_pending()}
    fn result_ptr(&mut self)->*mut (){
        let ptr = &mut self.result as *mut Poll<F::Output>;
        ptr as *mut ()
    }
    fn stack_poll(&mut self, cx: &mut Context<'_>,mut stack: NonNull<DynamicStack>)->FrameState{
        //this struct must be always pinned
        if self.result.is_ready() {return FrameState::Ready;} //safety check, to not poll future after finish

        with_stack(stack,||{
            let pinned = unsafe{
                stack.as_mut().clear_flag();
                Pin::new_unchecked(&mut self.future)
            };
            self.result = pinned.poll(cx);
        });
        if unsafe{stack.as_mut().clear_flag()} {
            FrameState::NewFrameCreated
        } else {
            if self.result.is_ready() {FrameState::Ready} else {FrameState::AsyncYield}
        }
    }

}

//virtual table
struct FrameVTable{
    stack_poll: fn(*mut (),&mut Context<'_>,NonNull<DynamicStack>)->FrameState,
    layout: Layout,
}

fn calc_layout<F: Future>()->Layout{
    todo!()
}

#[repr(C)]
struct UnsizedFrame{
    prev: Option<NonNull<DynamicStackFrame>>,
    vtable: &'static FrameVTable,

}
#[repr(C)]
struct SizedFrame<F: Future>{
    prev: Option<NonNull<DynamicStackFrame>>,
    vtable: &'static FrameVTable,
    result: Poll<F::Output>,
    future: F,
}