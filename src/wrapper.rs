
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};
use super::stack::DynamicStack;
use super::frame::FrameState;
use crate::frame::DynamicStackFrame;


pub struct HeapRecursive<F: Future>{
    stack: DynamicStack,
    _phantom: PhantomData<F>,
}
impl<F: Future> Unpin for HeapRecursive<F>{}
unsafe impl<F:Future + Send> Send for HeapRecursive<F>{}
unsafe impl<F:Future + Sync> Sync for HeapRecursive<F>{}

impl<F: Future> HeapRecursive<F>{
    pub fn wrap(future: F)->Self{
        let mut stack = DynamicStack::new();
        unsafe{stack.push_frame::<()>(DynamicStackFrame::new(future));}
        Self{stack,_phantom: PhantomData}
    }
}
impl<F: Future> Future for HeapRecursive<F>{
    type Output = F::Output;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        //we don't really care if this struct is pinned or not
        let stack = unsafe{ &mut self.as_mut().get_unchecked_mut().stack };

        assert!(!stack.is_empty(),"Error: HeapRecursive future was already polled to finish.");

        loop{
            match stack.stack_poll_top(cx) {
                //new frame was added and current frame yielded
                FrameState::NewFrameCreated => continue,//poll top frame once again
                FrameState::AsyncYield => return Poll::Pending,
                FrameState::Ready => {
                    if stack.has_single_frame() {
                        let mut frame = stack.pop_frame();
                        return unsafe{frame.take_result()}
                    } else {
                        match stack.stack_poll_prev(cx) {
                            FrameState::NewFrameCreated => continue,//frame was popped and other frame was pushed
                            FrameState::AsyncYield => return Poll::Pending, //frame was popped
                            FrameState::Ready => continue, //frame was popped
                        }
                    }
                }
            }
        }
    }
}