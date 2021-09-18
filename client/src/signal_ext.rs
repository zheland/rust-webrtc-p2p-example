use core::cell::RefCell;

use async_std::sync::Arc;
use sycamore::prelude::*;

pub trait SignalVecPush {
    type Item;

    fn push(&self, value: Self::Item);
}

pub trait SignalVecRemoveByPtrEq {
    type Item;

    fn remove_by_ptr_eq(&self, value: &Self::Item);
}

impl<T> SignalVecPush for Signal<RefCell<Vec<T>>> {
    type Item = T;

    fn push(&self, value: Self::Item) {
        let cell = self.get();
        let mut vec = cell.borrow_mut();
        vec.push(value);
        drop(vec);
        self.trigger_subscribers();
    }
}

impl<T> SignalVecRemoveByPtrEq for Signal<RefCell<Vec<Arc<T>>>> {
    type Item = Arc<T>;

    fn remove_by_ptr_eq(&self, value: &Self::Item) {
        let cell = self.get();
        let mut vec = cell.borrow_mut();

        let j = vec
            .iter()
            .position(|other| Arc::ptr_eq(value, other))
            .unwrap();
        drop(vec.remove(j));

        drop(vec);
        self.trigger_subscribers();
    }
}
