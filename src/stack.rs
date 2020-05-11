use core::marker::PhantomData;
use super::frame::*;
use core::task::*;
use core::future::Future;
use core::ptr::*;
use std::ops::DerefMut;

pub struct ResultRef<R>{
    pinned_frame: NonNull<DynamicStackFrame>,
    _phantom: PhantomData<R>,
}
impl<R> Copy for ResultRef<R>{}
impl<R> Clone for ResultRef<R>{
    fn clone(&self) -> Self {
        *self
    }
}

//singly linked list with additional runtime flags
pub struct DynamicStack{
    stack_top: Option<NonNull<DynamicStackFrame>>,
    flag: bool,
}

impl Drop for DynamicStack{
    fn drop(&mut self) {
        let mut ptr = self.stack_top;
        while let Some(mut opt) = ptr {
            unsafe {
                ptr = opt.as_mut().get_prev();
                //drop frame
                drop(Box::from_raw(opt.as_ptr()))
            }
        }
        self.stack_top = None;//maybe not necessary
    }
}


impl DynamicStack{
    #[inline]
    pub fn new()->Self{
        Self{
            stack_top: None,
            flag: false,
        }
    }
    #[inline]
    pub fn set_flag(&mut self){self.flag = true;}
    #[inline]
    pub fn clear_flag(&mut self)->bool{
        let prev = self.flag;
        self.flag = false;
        prev
    }
    #[inline]
    pub fn as_nonnull(&mut self)->NonNull<Self>{unsafe{NonNull::new_unchecked(self as *mut Self)}}

    //unsafe cause this function will erase information about lifetime of future
    #[inline]
    pub unsafe fn push_frame<T>(&mut self,mut frame: DynamicStackFrameBox)->ResultRef<T>{
        frame.set_prev(self.stack_top);
        let ptr = frame.into_raw();
        self.stack_top = Some(ptr);
        ResultRef{
            pinned_frame: ptr,
            _phantom: PhantomData,
        }
    }
    #[inline]
    pub fn is_empty(&self)->bool{ self.stack_top.is_none() }
    #[inline]
    pub fn has_single_frame(&self)->bool{
        match self.stack_top {
            Some(ptr) => unsafe{ ptr.as_ref().get_prev().is_none() },
            None => false,
        }
    }
    pub fn get_result<R>(&mut self,mut res: ResultRef<R>)->Poll<R>{
        unsafe{ res.pinned_frame.as_mut().take_result() }
    }
    pub fn stack_poll_top(&mut self,cx: &mut Context<'_>)->FrameState{
        let this = self.as_nonnull();
        let mut ptr = self.stack_top.expect("Error: Stack is empty, expected at least 1 frame.");
        unsafe{ptr.as_mut().stack_poll(cx,this)}
    }

    pub fn stack_poll_prev(&mut self,cx: &mut Context<'_>)->FrameState{
        let this = self.as_nonnull();
        unsafe{
            let ptr = self.stack_top.expect("Error: Stack is empty, expected at least 2 frames.");
            let mut ptr = ptr.as_ref().get_prev().expect("Error: Stack has 1 frame, expected at least 2.");
            ptr.as_mut().stack_poll(cx,this)
        }
    }
    pub fn pop_frame(&mut self)->DynamicStackFrameBox{
        let mut pop = unsafe{
            //pointer was allocated as box
            DynamicStackFrameBox::from_raw(self.stack_top.expect("Error: Popping from empty stack."))
        };
        self.stack_top = pop.get_prev();
        pop.set_prev(None);//don't leak pointer outside stack (maybe not necessary)
        pop
    }
}