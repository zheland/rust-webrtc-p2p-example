use core::fmt::{Debug, Formatter, Result};
use core::future::Future;
use core::pin::Pin;

// pub type BoxAsyncFn2<T1, T2, R> =
//     Box<dyn Fn(T1, T2) -> Pin<Box<dyn Future<Output = R> + Send + 'static>> + Send + Sync>;
pub type BoxAsyncFn2<T1, T2, R> = Box<dyn Fn(T1, T2) -> Pin<Box<dyn Future<Output = R>>>>;

pub struct BoxAsyncFn2Wrapper<T1, T2, R>(pub BoxAsyncFn2<T1, T2, R>);

impl<T1, T2, R> Debug for BoxAsyncFn2Wrapper<T1, T2, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_tuple("FutureBoxFn1").field(&"...").finish()
    }
}
