#![cfg(all(feature = "blocking", feature = "async"))]

use std::marker::PhantomData;

use grafton_visca::{
    blocking, operation::AppliedOnly, operation::Targeted, profiles::PtzOpticsG2, r#async::Camera,
    r#async::Operation, r#async::Session,
};

fn main() {
    let _: PhantomData<blocking::Camera<PtzOpticsG2>> = PhantomData;
    let _: PhantomData<blocking::Session> = PhantomData;
    let _: PhantomData<Camera<PtzOpticsG2>> = PhantomData;
    let _: PhantomData<Session> = PhantomData;
    let _: PhantomData<Operation<AppliedOnly>> = PhantomData;
    let _: PhantomData<Operation<Targeted>> = PhantomData;
}
