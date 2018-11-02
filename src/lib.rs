use std::{
    marker::PhantomData
};


pub struct GALRef<'a, B: 'static> {
    borrowed: B,
    _ref: PhantomData<&'a mut u8>
}

impl<'max, L: 'static> GALRef<'max, L> {

    pub unsafe fn __get_wrongly_static_borrow(&self) -> &L {
        &self.borrowed
    }

    pub unsafe fn __get_wrongly_static_borrow_mut(&mut self) -> &mut L {
        &mut self.borrowed
    }

    pub fn new<S: 'max>(short_lived: S) -> GALRef<'max, L>
        where L: GALCast<'max, 'max, ValueShortLived=S>
    {
        let long_lived: L = unsafe { GALCast::into_bad_static(short_lived) };
        GALRef {
            borrowed: long_lived,
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


pub trait GALCast<'max: 'short,  'short>: 'static {
    type ValueShortLived: 'short;

    //it's not safe as the lifetime we cast to "could" be longer then the original lifetime if this
    //trait was not used correctly.
    unsafe fn cast(&'short self) -> &'short Self::ValueShortLived;

    unsafe fn cast_mut(&'short mut self) -> &'short mut Self::ValueShortLived;

    unsafe fn into_bad_static(value: Self::ValueShortLived) -> Self
        where Self::ValueShortLived: Sized, Self: Sized;
}


#[cfg(test)]
mod test {
    use std::mem;
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

    impl<'max: 'short, 'short> GALCast<'max, 'short> for Transaction<'static> {
        type ValueShortLived = Transaction<'short>;

        unsafe fn cast(&'short self) -> &'short Self::ValueShortLived {
            mem::transmute::<
                &'short Transaction<'static>,
                &'short Transaction<'short>
            >(self)
        }

        unsafe fn cast_mut(&'short mut self) -> &'short mut Self::ValueShortLived {
            mem::transmute::<
                &'short mut Transaction<'static>,
                &'short mut Transaction<'short>
            >(self)
        }

        unsafe fn into_bad_static(value: Self::ValueShortLived) -> Self
            where Self::ValueShortLived: Sized, Self: Sized
        {
            mem::transmute::<
                Transaction<'short>,
                Transaction<'static>
            >(value)
        }
    }

    #[test]
    fn it_works() {
        let mut conn = Connection::default();
        {
            let transaction = conn.transaction();
            let mut gal_ref: GALRef<Transaction<'static>> = GALRef::new(transaction);
            gal_ref.get_mut().inc();
        }
        assert_eq!(conn.count, 1);
    }
}