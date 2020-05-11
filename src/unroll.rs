
use core::pin::Pin;
use core::task::{Context, Poll};
use core::future::Future;
use super::context::*;
use super::stack::*;
use core::mem::replace;
use std::mem::{size_of, size_of_val};
use crate::frame::{DynamicStackFrame, DynamicStackFrameBox};
use std::marker::PhantomData;

#[repr(u8)]
enum UnrollState<T>{
    Idle(DynamicStackFrameBox),
    Executing(ResultRef<T>),
    Finished,
}

pub struct UnrollBox<'a,T>{
    state: UnrollState<T>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a,T> Unpin for UnrollBox<'a,T>{}


impl<'a,T> UnrollBox<'a,T>{
    #[inline]
    pub fn new<F: Future<Output=T> + 'a>(future: F)->Self{
        Self{
            state: unsafe{UnrollState::Idle(DynamicStackFrame::new(future))},
            _phantom: PhantomData,
        }
    }
}

impl<'a,T> Future for UnrollBox<'a,T>{
    type Output = T;
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe{
            let this =  self.as_mut().get_unchecked_mut();
            //set placeholder value
            let value = replace(&mut this.state,UnrollState::Finished);
            match value {
                UnrollState::Idle(fut) =>{
                    let mut stack = get_current_stack();
                    let res = stack.as_mut().push_frame(fut);
                    stack.as_mut().set_flag();
                    this.state = UnrollState::Executing(res);
                    Poll::Pending //recursion yield
                }
                UnrollState::Executing(val) => {
                    let mut stack = get_current_stack();
                    match stack.as_mut().get_result(val) {
                        Poll::Pending =>{
                            this.state = UnrollState::Executing(val);
                            Poll::Pending //normal yield
                        }
                        Poll::Ready(t) =>{
                            stack.as_mut().pop_frame();
                            //UnrollState::Finished is already set
                            Poll::Ready(t)
                        }
                    }
                }
                UnrollState::Finished =>{
                    panic!("Error: This UnrollBox future cannot be polled after finish.")
                }
            }
        }
    }
}