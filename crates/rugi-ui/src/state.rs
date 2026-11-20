//! Id scopes and typed retained state.

use crate::{Ui, UiError};
use rugi_core::Id;
use std::any::Any;

impl Ui {
    /// Current id scope.
    #[inline]
    pub fn current_id(&self) -> Id {
        self.id_stack.last().copied().unwrap_or(Id::ROOT)
    }

    /// Derives an id from `source` in the current scope.
    pub fn make_id(&self, source: &str) -> Id {
        self.current_id().with(source)
    }

    /// Pushes an id scope (e.g. per list row).
    pub fn push_id(&mut self, source: &str) {
        let id = self.make_id(source);
        self.id_stack.push(id);
    }

    /// Pushes an integer id scope.
    #[inline]
    pub fn push_id_u64(&mut self, v: u64) {
        let id = self.current_id().with_u64(v);
        self.id_stack.push(id);
    }

    /// Pops an id scope.
    pub fn pop_id(&mut self) {
        if self.id_stack.pop().is_none() {
            self.report(UiError::NothingToEnd);
        }
    }

    /// Runs `f` inside an id scope.
    pub fn scope<R>(&mut self, source: &str, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.push_id(source);
        let r = f(self);
        self.pop_id();
        r
    }

    /// Typed retained state for `id`, created with `Default` on first use.
    pub fn state_mut<T: Default + 'static>(&mut self, id: Id) -> &mut T {
        let value = self.states.entry(id).or_insert_with(|| Box::new(T::default()) as Box<dyn Any>);
        if !value.is::<T>() {
            *value = Box::new(T::default());
        }
        value.downcast_mut::<T>().expect("type checked above")
    }
}
