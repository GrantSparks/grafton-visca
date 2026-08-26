#![cfg(all(feature = "blocking", feature = "async"))]

use std::marker::PhantomData;

use grafton_visca::{
    blocking,
    completion::{AppliedOnly, Targeted},
    profiles::PtzOpticsG2,
    Camera, Operation, Session,
};

fn main() {
    let _: PhantomData<blocking::Camera<'static, PtzOpticsG2>> = PhantomData;
    let _: PhantomData<blocking::Session> = PhantomData;
    let _: PhantomData<Camera<PtzOpticsG2>> = PhantomData;
    let _: PhantomData<Session> = PhantomData;
    let _: PhantomData<Operation<AppliedOnly>> = PhantomData;
    let _: PhantomData<Operation<Targeted>> = PhantomData;
}
