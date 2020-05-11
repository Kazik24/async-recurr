
use core::cell::Cell;
use super::stack::*;
use core::ptr::NonNull;

//only because of this it depends on std!
std::thread_local!{
    static CONTEXT_MARK: Cell<Option<NonNull<DynamicStack>>> = Cell::new(None);
}

pub fn with_stack<R>(stack: NonNull<DynamicStack>,func: impl FnOnce()->R)->R{
    struct Cleaner(Option<NonNull<DynamicStack>>);
    impl Drop for Cleaner{
        fn drop(&mut self) {
            CONTEXT_MARK.with(move |mark|mark.set(self.0)); //ensure resetting mark if closure panics
        }
    }

    let prev = CONTEXT_MARK.with(move |mark|mark.replace(Some(stack)));
    let clean = Cleaner(prev);
    let result = func();
    drop(clean); // explicit run destructor
    result
}
pub fn get_current_stack()->NonNull<DynamicStack>{
    let ptr = CONTEXT_MARK.with(|mark| mark.get());
    ptr.expect("Error: Not invoked with recursive context.")
}