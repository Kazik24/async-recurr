
mod frame;
mod context;
mod stack;
mod wrapper;
mod unroll;


pub use wrapper::HeapRecursive;
pub use unroll::UnrollBox;

#[macro_export]
macro_rules! unroll {
    ($future:expr) => {
        {
            let temp = $crate::UnrollBox::new($future);
            temp
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    async fn fibonacci(n: u32)->u32{
        if n <= 1 { n }
        else{ unroll!(fibonacci(n-1)).await + unroll!(fibonacci(n-2)).await }
    }

    async fn ackermann(m: u64,n: u64) -> u64{
        if m == 0 {return n+1;}
        if m > 0 && n == 0 {
            return unroll!(ackermann(m-1,1)).await;
        }
        let nn = unroll!(ackermann(m,n-1)).await;
        return unroll!(ackermann(m-1,nn)).await;
    }

    #[test]
    pub fn test_fibonacci(){
        let result = block_on(HeapRecursive::wrap(fibonacci(30)));
        assert_eq!(result,832040);
    }

    #[test]
    pub fn test_ackermann(){
        let result = block_on(HeapRecursive::wrap(ackermann(3,8)));
        assert_eq!(result,2045);
    }

}
