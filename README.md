# Galemu &emsp; [![LatestVersion]][crates.io] [![RustcVersion1.30+]][rustc_ver]

[LatestVersion]: https://img.shields.io/crates/v/galemu.svg
[crates.io]: https://crates.io/crates/galemu
[RustcVersion1.30+]: https://img.shields.io/badge/rustc-1.30+-lightgray.svg
[rustc_ver]: https://blog.rust-lang.org/2018/10/25/Rust-1.30.0.html


**galemu is a library to make it easy to work around not yet having generic associated types (GAT) wrt. lifetimes (GAL)**

---


In current rust it can be tricky to abstract over some types due to not
having generic associated types (GAT) in rust (yet). Often it is just
necessary to have GAT wrt. liftimes and this crate provides some helpers
to work around this.

# Example Problem

For example you want to abstract over a (db) connection being able to
open a transaction and that transaction being commitable. Once rust
has GAT you would write following (stringly simplified, e.g. without
returning results, additional methods etc.):

```rust
trait GenericConnection {
    type Transaction<'conn>: GenericTransaction;

    // the lifetime could have been omitted
    fn create_transaction<'s>(&'s self) -> Self::Transaction<'s>;
}

trait GenericTransaction {
    fn commit(self);
}
```

And then you could implement `GenericTransaction` for any `Transaction<'a>`.

But today you can only have following:

```rust
trait GenericConnection {
    type Transaction: GenericTransaction;

    // the lifetime could have been omitted
    fn create_transaction(&self) -> Self::Transaction;
}

trait GenericTransaction {
    fn commit(self);
}
```

But if you want to implement it for a `Transaction<'conn>` you can't as
the `'conn` needs to be bound to the lifetime of the `create_transaction`
function.


# Problem workaround

The idea of this crate is to lift the liftime into a know wraper type
resulting in following setup:

```rust
use galemu::{Bound, BoundExt};

trait GenericConnection {
    type Transaction: GenericTransaction;

    // the lifetime could have been omitted
    fn create_transaction<'s>(&'s self) -> Bound<'s, Self::Transaction>;
}

trait GenericTransaction: for<'a> BoundExt {
    // on nightly use the "arbitrary self type" feature
    fn commit(me: Bound<'s, Self>);
}
```

And for implementing it you would use following:

```rust
use galemu::{create_gal_wrapper_type};

create_gal_wrapper_type!{
    /// Wraps `Transaction` erasing it's lifetime.
    ///
    /// This can only be used through a `Bound<'a, TransactionWrapper>` instance,
    /// as only then it is possible to access the wrapped type with the correct lifetime.
    struct TransactionWrapper(Transaction<'a>);
}

impl GenericConnection for Connection {
    type Transaction = TransactionWrapper;

    fn create_transaction(&mut self) -> Bound<Self::Transaction> {
        let trans = self.transaction();
        TransactionWrapper::new(trans)
    }
}
```

You can take a look at the [module level documentation](https://docs.rs/galemu) for a full example.
