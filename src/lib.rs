//! Create for helping you to workaround not having generic associated lifetimes.
//!
//! # Problem Case
//!
//! Lets say you need to abstract over a connection/transaction of some form.
//! A intuitive way would be to have something like following code (which
//! for simplicity omits all the results and for this example irrelevant
//! methods):
//!
//! ```
//! trait GeneralConnection {
//!     type Transaction: GeneralTransaction;
//!     fn create_transaction(&mut self) -> Self::Transaction;
//! }
//!
//! trait GeneralTransaction {
//!     // Potential results ommited.
//!     fn commit(self);
//!     fn abort(self);
//! }
//! ```
//!
//! The problem with this is that most transactions have a signature of the form
//! `Transaction<'conn>` where `'conn` binds to the self parameter of the method
//! used to construct the transaction. _This currently can not be represented in rust_.
//!
//! In the _future_ you might be able to use generic associated types (GAT) or the subset
//! limited to lifetimes of it (GAL). This _would_ allow following code:
//!
//! ```ignore
//! trait GeneralConnection {
//!     type Transaction<'conn>: GeneralTransaction;
//!     // This don't work on current rust (1.30) not even in nightly.
//!     // Lifetimes could have been omitted.
//!     fn create_transaction<'conn>(&'conn mut self) -> Self::Transaction<'conn>;
//! }
//!
//! trait GeneralTransaction {
//!     // Potential results ommited.
//!     fn commit(self);
//!     fn abort(self);
//! }
//! ```
//!
//!
//! # Problem Circumvention
//!
//! This crate provides a patterns (and a small helper type, trait and macro) to circumvent this
//! limitation. Note that while possible it's not necessary a nice solution.
//!
//! The idea of this create is to lift the lifetime from the type parameter into a know wrapper
//! type, this produces following code:
//!
//! ```
//! use galemu::{Bound, BoundExt};
//!
//! trait GeneralConnection {
//!     type Transaction: GeneralTransaction;
//!     // Lifetimes could have been omitted
//!     fn create_transaction<'conn>(&'conn mut self) -> Bound<'conn, Self::Transaction>;
//! }
//!
//! trait GeneralTransaction: for<'a> BoundExt<'a> {
//!     // Potential results omitted.
//!     // Once the rust "arbitrary self types" features lands on stable this can
//!     // be made much nicer (by using `self: Bound<Self>`).
//!     fn commit<'c>(me: Bound<'c, Self>);
//!     fn abort<'c>(me: Bound<'c, Self>);
//! }
//! ```
//!
//! Note that `Bound` has some very specific safety guarantees about how it binds the
//! `'conn` lifetime to `Self::Transaction` and that the `GeneralTransaction` now
//! accepts a lifetime bound Self. Also not that without the unstable "arbitrary self type"
//! feature the methods will no longer have a self parameter so they will need to be
//! called with `GeneralTransaction::commit(trans)` instead of `trans.commit()`.
//!
//! The trick is that now if you need to implement `GeneralConnection` for a
//! with transactions of the form `Transaction<'conn>` you can approach it
//! in following way:
//!
//! 1. Wrap the transaction type into one mich contains a `ManualDrop<UnsafeCell<Transaction<'static>>`.
//!    We call the type `TransactionWrapper`.
//! 2. The `create_transaction(&'s mut self)` method will now internal create a transaction with the
//!    signature `Transaction<'s>` wrap it into a `UnsafeCell` and then transmute it to `'static` erasing
//!    the original lifetime (we call the wr).
//! 3. To still keep the original lifetime `'s` a `Bound<'s, TransactionWrapper>` is returned.
//! 4. The methods on `GeneralTransaction` accept a `Bound<'c, Self>` where, due to the constraints
//!    of `Bound` `'c` is guaranteed to be a "subset" of `'s` (as where constraint this is `'s: 'c`).
//!    So in the method we can turn the transaction back into the appropriate lifetime.
//! 5. On drop we manually drop the `Transaction<'static>` in the [`BoundExt::pre_drop()`] call _instead
//!    of the normal drop call_, also we do so after turning it back to the right lifetime.
//! 6. For usability `TransactionWrapper` should contain methods to get `&`/`&mut` of the correct inner
//!    type from a `&`/`&mut` to a `Bound<TransactionWrapper>`.
//!
//! Note that some of the aspects (like the part about `ManualDrop` and `pre_drop`) might seem arbitrary
//! but are needed to handle potential specialization of code based on the `'static` lifetime.
//!
//! # What this lib provides:
//!
//! 1. The [`Bound`] type for binding the lifetime to the part where it was erased in a safe way
//!    with some safety guarantees which go above a normal wrapper.
//! 2. The [`BoundExt`] trait needed to handle drop wrt. to some specialization edge cases.
//! 3. The [`create_gal_wrapper_type_for`] which implements all unsafe code for
//!    you.
//!
//! # Example
//!
//! ```
//! use galemu::{Bound, BoundExt, create_gal_wrapper_type};
//!
//! struct Connection {
//!     count: usize
//! }
//!
//! struct Transaction<'conn> {
//!     conn: &'conn mut Connection
//! }
//!
//! impl Connection {
//!     fn transaction(&mut self) -> Transaction {
//!         Transaction { conn: self }
//!     }
//! }
//!
//! trait GCon {
//!     type Transaction: GTran;
//!
//!     fn create_transaction(&mut self) -> Bound<Self::Transaction>;
//! }
//!
//! trait GTran: for<'s> BoundExt<'s> {
//!     fn commit<'s>(me: Bound<'s, Self>);
//!     fn abort<'s>(me: Bound<'s, Self>);
//! }
//!
//! create_gal_wrapper_type!{ struct TransWrap(Transaction<'a>); }
//!
//! impl GCon for Connection {
//!     type Transaction = TransWrap;
//!
//!     fn create_transaction(&mut self) -> Bound<Self::Transaction> {
//!         let transaction = self.transaction();
//!         TransWrap::new(transaction)
//!     }
//! }
//!
//! impl GTran for TransWrap {
//!     fn commit<'s>(me: Bound<'s, Self>) {
//!         let trans = TransWrap::into_inner(me);
//!         trans.conn.count += 10;
//!     }
//!
//!     fn abort<'s>(me: Bound<'s, Self>) {
//!         let trans = TransWrap::into_inner(me);
//!         trans.conn.count += 3;
//!     }
//! }
//!
//! fn create_commit_generic(x: &mut impl GCon) {
//!     let trans = x.create_transaction();
//!     // Using arbitrary self types this can become `trans.commit()`.
//!     // (extension traits for `Bound<T> where T: GTran` are also an option here).
//!     GTran::commit(trans)
//! }
//!
//! fn create_abort_specific(x: &mut Connection) {
//!     let trans = x.create_transaction();
//!     GTran::abort(trans)
//! }
//!
//! #[test]
//! fn it_can_be_used() {
//!     let mut conn = Connection { count: 0 };
//!     {
//!         create_commit_generic(&mut conn);
//!     }
//!     {
//!         create_abort_specific(&mut conn);
//!     }
//!     assert_eq!(conn.count, 13)
//! }
//! ```
#![deny(unsafe_code)]

use std::{
    marker::PhantomData,
    ops::Deref,
    mem, ptr
};

#[macro_use]
mod macros;

/// Workaround for rust not having generic associated lifetimes (GAT/GAL).
///
/// # General Safety Aspects
///
/// `Bound<'a, T>` binds a lifetimes `'a` to a instance of a type `T`.
/// It guarantees on a safety level that rebinding to any lifetimes which
/// is not a subset of the original lifetime requires unsafe code (as it
/// _is_ unsafe).
///
/// This guarantee allows libraries to accept a `Bound<'a, ThereType>`
/// instead of self an rely on the `'a` lifetime for unsafe code.
///
/// For example `ThereType` could contain a `Transaction<'static>` which
/// was transmuted from a `Transaction<'a>` but due to it needing to use
/// it as a associated type it can't have the lifetime `'a`. Still as
/// the method  accepts a `Bound<'a, ThereType>` they can rely on the
/// `'a` not having been changed from the original construction and
/// as such know that they can use it after casting/transmuting it
/// to a `Transaction<'a>`;
///
///
/// # Drop
///
/// This method will call `pre_drop(&mut Bound<'a, Self>)` when the wrapper is
/// dropped (which will be followed by a call to `Self::drop` is Self impl. drop).
///
/// As `Self` might contain a type with a wrong lifetime (`'static`) and rust will/might
/// have _the feature to specialize Drop on `'static`_. We can not drop that inner value
/// on `Drop` safely (we can't turn it to "any" shorter lifetime either as it might have
/// been static and as such might need to run the specialized code). So we have to drop
/// it while we still have access to the original
///
pub struct Bound<'a, T: BoundExt<'a>> {
    limiter: PhantomData<&'a mut T>,
    inner: T
}

impl<'a, T> Bound<'a, T>
    where T: BoundExt<'a>
{

    /// Creates a new `Bound` instance.
    ///
    /// # Safety
    ///
    /// A instance of `T` has to be bound to a lifetime valid
    /// for it as `Bound` gives this guarantee. So using `new`
    /// with a instance of a type corresponding to a different
    /// lifetime then `'a` is unsafe behavior.
    ///
    /// Also note that using this method can have **other safety
    /// constraints defined by `T`** as such it _should_ only be
    /// used by `T` to create a `Bound` wrapper of itself.
    #[allow(unsafe_code)]
    pub unsafe fn new(inner: T) -> Self {
        Bound {
            limiter: PhantomData,
            inner
        }
    }

    /// Get `&mut` access to the inner type.
    ///
    /// # Safety
    ///
    /// Implementors of traits used for `T` might rely on
    /// the instance of `T` being coupled with the right
    /// lifetime `'a`. But a `&mut` borrow would allow
    /// switching the content of two `Bound` instances, which
    /// might brake safety constraints.
    #[allow(unsafe_code)]
    pub unsafe fn _get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consumes self the return the contained instance of `T`.
    ///
    /// # Safety / Drop
    ///
    /// As dropping some thinks can only be safely done with
    /// [`BoundExt::pre_drop()`] turning this instance into `T`
    /// might cause the leakage of some resources and should
    /// only be done by methods which are aware of this problems.
    pub fn _into_inner(mut self) -> T {
        // workaround for having no "no-drop" destruction
        let inner = {
            let &mut Bound { limiter:_, ref mut inner } = &mut self;
            unsafe_block! {
                "self is forgotten after inner was moved out" => {
                    ptr::read(inner as *mut T)
                }
            }
        };
        mem::forget(self);
        inner
    }
}

impl<'a, T> Deref for Bound<'a, T>
    where T: BoundExt<'a>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}


impl<'a, T> Drop for Bound<'a, T>
    where T: BoundExt<'a>
{
    fn drop(&mut self) {
        unsafe_block! {
            "after this drop call rust will call drop on all members" => {
                BoundExt::pre_drop(self)
            }
        }
    }
}

pub trait BoundExt<'a>: 'a + Sized {

    /// Called when dropping the `Bound` wrapper before dropping the inner value.
    ///
    /// # Safety/Drop
    ///
    /// Due to possible specialization of `Drop` on `'static` dropping a "fake"
    /// static value in `Self` might not be safe at all. This can be handled by
    /// making it non `'sstatic` and explicitly dropping it during the `Self::drop`
    /// call, but then this wrapper _migth_ have been `'static` so using the
    /// `'static` specialization might have been more correct.
    ///
    /// This method allows to instead explicitly drop the possible "fake" `'static`
    /// value in `Self` earlier on drop while we still have the correct lifetime.
    ///
    /// # Call Safety
    ///
    /// This method is only meant to be called when dropping the `Bound` wrapper,
    /// i.e. immediately before calling `Self::drop`. Calling anything expect
    /// [`Drop::drop`][1] after calling [`BoundExt::pre_drop()`] is unsafe, so
    /// the caller has to make sure that this won't happen.
    ///
    /// Normally this should **only be called by the `Bound` `Drop` implementation**.
    #[allow(unsafe_code)]
    unsafe fn pre_drop(_me: &mut Bound<'a, Self>) {}
}

/// Creates a wrapper type for a type with a single lifetime parameter lifting the lifetime to `Bound`.
///
/// The new type will have:
/// - A safe `new` method accepting a instance of the wrapped type with a lifetime
///   `'a` and returns a `Bound<'a, WrapperType>`.
/// - Impl for `BoundExt` incl, `BoundExt::pre_drop` (the wrapper doesn't need a `Drop` impl.).
/// - A `get` function which accept `&Bound<'a, WrapperType>` and returns a `&WrappedType<'a>`.
/// - A `get_mut` function which accepts `&mut Bound<'a, WrapperType>` and returns a `&mut WrappedType<'a>`.
/// - A `into_inner` function which accpets a `Bound<'a, WrapperType>` and returns a `WrappedType<'a>`.
///
/// Note that all the above functions are implemented on the wrapper type, i.e. you can't be
/// generic over them (at last not without generic associated lifetimes).
///
///
/// # Example
///
/// See module level documentation.
#[macro_export]
macro_rules! create_gal_wrapper_type {

    ( $(#[$attr:meta])* $v:vis struct $Type:ident ($Inner:ident<$lt:tt>); ) => (

        $(#[$attr])*
        $v struct $Type {
            static_cell: ::std::mem::ManuallyDrop<::std::cell::UnsafeCell<$Inner<'static>>>
        }

        impl $Type {

            /// Create a new "bound" instance of this type.
            ///
            /// This will lift the lifetime from the inner type to the `Bound` wrapper,
            /// wrapping the inner type into this type while erasing it's lifetime
            $v fn new<$lt>(value: $Inner<$lt>) -> $crate::Bound<$lt, Self> {
                use std::{ mem::{self, ManuallyDrop}, cell::UnsafeCell };

                let cell = ManuallyDrop::new(UnsafeCell::new(value));
                $crate::unsafe_block! {
                    "same mem layout, the unsafe cell contains the wrong lifetime in check" => {
                        let static_cell = mem::transmute(cell);
                        Bound::new($Type { static_cell })
                    }
                }
            }

            #[allow(unused)]
            $v fn get<'s, 'b: 's>(me: &'b Bound<'s, Self>) -> &'b $Inner<'s> {
                let ptr: *const $Inner<'static> = me.static_cell.get();
                $crate::unsafe_block! {
                    "Self was transmuted from $Inner and `'s` is valid due to Bound's guarantees" => {
                        let as_ref: &'b $Inner<'static> = &*ptr;
                        ::std::mem::transmute(as_ref)
                    }
                }
            }

            #[allow(unused)]
            $v fn get_mut<'s, 'b: 's>(me: &'b mut Bound<'s, Self>) -> &'b mut $Inner<'s> {
                let ptr: *mut $Inner<'static> = me.static_cell.get();
                $crate::unsafe_block! {
                    "Self was transmuted from $Inner and `'s` is valid due to Bound's guarantees" => {
                        let as_mut: &'b mut $Inner<'static> = &mut *ptr;
                        ::std::mem::transmute(as_mut)
                    }
                }
            }

            #[allow(unused)]
            $v fn into_inner<'s>(me: Bound<'s, Self>) -> $Inner<'s> {
                use std::{ mem::{self, ManuallyDrop}, cell::UnsafeCell };

                let $Type { static_cell } = me._into_inner();

                let non_static_cell = $crate::unsafe_block! {
                    "the $Inner<'static> originally had been a $Inner<'s>" => {
                        mem::transmute::<
                            ManuallyDrop<UnsafeCell<$Inner<'static>>>,
                            ManuallyDrop<UnsafeCell<$Inner<'s>>>
                        >(static_cell)
                    }
                };

                ManuallyDrop::into_inner(non_static_cell).into_inner()
            }
        }

        impl<'a> $crate::BoundExt<'a> for $Type {

            #[allow(unsafe_code)]
            unsafe fn pre_drop(me: &mut $crate::Bound<'a, Self>) {
                use std::{mem::{self, ManuallyDrop}, cell::UnsafeCell};

                // Safe due to the constraints of only calling drop after pre_drop
                let static_as_mut: &mut ManuallyDrop<UnsafeCell<$Inner<'static>>> = &mut me._get_mut().static_cell;
                let as_mut: &mut ManuallyDrop<UnsafeCell<$Inner<'a>>> = mem::transmute(static_as_mut);
                ManuallyDrop::drop(as_mut)
            }
        }

    );
}


#[cfg(test)]
mod test {
    use super::*;

    struct Connection {
        count: usize
    }

    struct Transaction<'conn> {
        conn: &'conn mut Connection
    }

    impl Connection {
        fn transaction(&mut self) -> Transaction {
            Transaction { conn: self }
        }
    }

    trait GCon {
        type Transaction: GTran;

        fn create_transaction(&mut self) -> Bound<Self::Transaction>;
    }

    trait GTran: for<'s> BoundExt<'s> {
        fn commit<'s>(me: Bound<'s, Self>);
        fn abort<'s>(me: Bound<'s, Self>);
    }

    create_gal_wrapper_type!{
        /// Wraps `Transaction` erasing it's lifetime.
        ///
        /// This can only be used through a `Bound<'a, TransWrap>` instance,
        /// as only then it is possible to access the wrapped type with the
        /// correct lifetime
        struct TransWrap(Transaction<'a>);
    }

    impl GCon for Connection {
        type Transaction = TransWrap;

        fn create_transaction(&mut self) -> Bound<Self::Transaction> {
            let transaction = self.transaction();
            TransWrap::new(transaction)
        }
    }

    impl GTran for TransWrap {
        fn commit<'s>(me: Bound<'s, Self>) {
            let trans = TransWrap::into_inner(me);
            trans.conn.count += 10;
        }

        fn abort<'s>(me: Bound<'s, Self>) {
            let trans = TransWrap::into_inner(me);
            trans.conn.count += 3;
        }
    }

    fn create_commit_generic(x: &mut impl GCon) {
        let trans = x.create_transaction();
        // Using arbitrary self types this can become `trans.commit()`.
        // (extension traits for `Bound<T> where T: GTran` are also an option here).
        GTran::commit(trans)
    }

    fn create_abort_specific(x: &mut Connection) {
        let trans = x.create_transaction();
        GTran::abort(trans)
    }

    #[test]
    fn it_can_be_used() {
        let mut conn = Connection { count: 0 };
        {
            create_commit_generic(&mut conn);
        }
        {
            create_abort_specific(&mut conn);
        }
        assert_eq!(conn.count, 13)
    }
}