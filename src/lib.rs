//! This library provides a `GALRef` type for working around not having generic associated lifetimes (GAL).
//!
//! The idea behind this type is to lift the lifetime from some type (e.g.
//! `Transaction<'conn>`) into a well known wrapper type (e.g. `GALRef<'conn, Transaction<'static>`).
//!
//! # Example
//!
//!
use std::{
    marker::PhantomData,
    cell::UnsafeCell
};

/// Type used to lift a lifetime from a associated type to a known wrapper type.
///
/// The idea behind this type is to lift the lifetime from some type (e.g.
/// `Transaction<'conn>`) into a well known wrapper type (e.g. `GALRef<'conn, Transaction<'static>`).
///
/// This allows to workaround cases where generic associated lifetimes should be used, through
/// can't as rust does not support them yet. E.g. if you have a associated type which is returned
/// from a function but should borrow the functions `self` parameter with a lifetime local to that
/// function instead of bound to the trait. See the module level documentation for an example.
///
pub struct GALRef<'a, L: 'static> {
    borrowed: UnsafeCell<L>,
    _ref: PhantomData<&'a mut u8>
}

impl<'max, L: 'static> GALRef<'max, L> {

    pub unsafe fn __get_wrongly_static_borrow(&self) -> &L {
        & *self.borrowed.get()
    }

    pub unsafe fn __get_wrongly_static_borrow_mut(&mut self) -> &mut L {
        &mut *self.borrowed.get()
    }

    pub fn new<S: 'max>(short_lived: S) -> GALRef<'max, L>
        where L: GALCast<'max, 'max, ValueShortLived=S>
    {
        let long_lived: L = unsafe { GALCast::into_bad_static(short_lived) };
        GALRef {
            borrowed: UnsafeCell::new(long_lived),
            _ref: PhantomData
        }
    }

    pub fn get<'short>(&'short self) -> &'short <L as GALCast<'max, 'short>>::ValueShortLived
        where L: GALCast<'max, 'short>
    {
        unsafe { self.__get_wrongly_static_borrow().cast() }
    }

    pub fn get_mut<'short>(&'short mut self) -> &'short mut <L as GALCast<'max, 'short>>::ValueShortLived
        where L: GALCast<'max, 'short>
    {
        unsafe { self.__get_wrongly_static_borrow_mut().cast_mut() }
    }
}


/// Trait a type needs to implement to support [`GALRef`].
///
/// # Safety
///
/// The constraint `'max: 'short` makes sure that the cast type from
/// [`GALCast::cast()`] and [`GALCast::cast_must()`] can not outlive
/// the original lifetime, through this is only guaranteed if this trait
/// is implemented correctly.
///
/// A correct implementation means that `ValueShortLived` needs to correspond
/// exactly to the lifetime `'short` instead of matching exactly or outliving
/// it (through it can outlive it wrt. coercion etc.). What I mean if you
/// impl `GALCast` for a `Transaction<'static>` then `ValueShortLived` should
/// be `Transaction<'short>`. Using `Transaction<'static>` for `ValueShortLived`
/// might compile but is most likely NOT safe, which is why this trait is unsafe.
pub unsafe trait GALCast<'max: 'short,  'short>: 'static {
    type ValueShortLived: 'short;

    /// This function converts a type of a "bad" long lifetimes to a shorter one.
    ///
    /// For example it might turn a `&'short Transaction<'static>` into a `&'short Transaction<'short>`.
    fn cast(&'short self) -> &'short Self::ValueShortLived;

    /// This function converts a type of a "bad" long lifetimes to a shorter one.
    ///
    /// For example it might turn a `&'short mut Transaction<'static>` into a `&'short mut Transaction<'short>`.
    fn cast_mut(&'short mut self) -> &'short mut Self::ValueShortLived;

    /// This function turns a normal type with a lifetime in a `'static` version of it.
    ///
    /// For example it might convert a `Transaction<'a>` into a `Transaction<'static>`.
    ///
    /// # Safety
    ///
    /// This conversion is normally not correct as using the resulting  `'static` type
    /// can potentially be hazardous as it might outlive elements it borrowed. But the
    /// way it is used in `GALRef` makes sure that it is only used after casting it back
    /// to a lifetime which *is outlived by* it's original lifetime, making it safe to
    /// use in _that_ context.
    unsafe fn into_bad_static(value: Self::ValueShortLived) -> Self
        where Self::ValueShortLived: Sized, Self: Sized;
}


/// Implements `GALCast` for a type having only single lifetime as generic parameter.
#[macro_export]
macro_rules! impl_gal_cast {
    ($type:ident < $_lt:tt >) => (
        unsafe impl<'max: 'short, 'short> GALCast<'max, 'short> for $type<'static> {
            type ValueShortLived = $type<'short>;

            fn cast(&'short self) -> &'short Self::ValueShortLived {
                unsafe {
                    ::std::mem::transmute::<
                        &'short $type<'static>,
                        &'short $type<'short>
                    >(self)
                }
            }

            fn cast_mut(&'short mut self) -> &'short mut Self::ValueShortLived {
                unsafe {
                    ::std::mem::transmute::<
                        &'short mut $type<'static>,
                        &'short mut $type<'short>
                    >(self)
                }
            }

            unsafe fn into_bad_static(value: Self::ValueShortLived) -> Self
                where Self::ValueShortLived: Sized, Self: Sized
            {
                ::std::mem::transmute::<
                    $type<'short>,
                    $type<'static>
                >(value)
            }
        }
    );
}


#[cfg(test)]
mod test {
    use std::{
        io::Error
    };
    use super::*;


    #[derive(Default)]
    struct Connection {
        count: u32
    }

    struct Transaction<'conn> {
        conn: &'conn mut Connection
    }

    impl<'conn> Transaction<'conn> {
        fn inc(&mut self) {
            self.conn.count += 1;
        }
    }

    impl Connection {

        fn transaction(&mut self) -> Transaction {
            Transaction { conn: self }
        }
    }

    impl_gal_cast!{ Transaction<'conn> }

    #[test]
    fn gal_ref_can_be_used() {
        let mut conn = Connection::default();
        {
            let transaction = conn.transaction();
            let mut gal_ref: GALRef<Transaction<'static>> = GALRef::new(transaction);
            gal_ref.get_mut().inc();
        };
        assert_eq!(conn.count, 1);
    }

    trait GeneralTransaction {
        fn commit(&mut self) -> Result<(), Error>;
    }

    trait GeneralConnection {
        type Transaction: GeneralTransaction;

        fn create_transaction(&mut self) -> Result<GALRef<Self::Transaction>, Error>;
    }

    impl<'conn> GeneralTransaction for Transaction<'conn> {
        fn commit(&mut self) -> Result<(), Error> {
            self.conn.count += 10;
            Ok(())
        }
    }

    impl GeneralConnection for Connection {
        type Transaction = Transaction<'static>;

        fn create_transaction(&mut self) -> Result<GALRef<Self::Transaction>, Error> {
            let transaction = self.transaction();
            Ok(GALRef::new(transaction))
        }
    }

    /// this won't work
    // fn create_and_commit_transaction<'l, C>(x: &'l mut C) -> Result<(), Error>
    //     where C: GeneralConnection,
    //         C::Transaction: for<'s> GALCast<'l,'s>
    // {
    //     let transaction = x.create_transaction()?;
    //     transaction.get_mut().commit()?;
    //     Ok(())
    // }

    fn non_generic_create_and_commit_transaction(c: &mut Connection) -> Result<(), Error> {
        let mut transaction = c.create_transaction()?;
        transaction.get_mut().commit()?;
        Ok(())
    }

    #[test]
    fn is_usable_in_function() {
        let mut conn = Connection::default();
        non_generic_create_and_commit_transaction(&mut conn).unwrap();
        assert_eq!(conn.count, 10);
    }

    //TODO compiler test?
}