use grafton_visca::{StateCache, StateEntry, StateKey};

fn inspect_cache(cache: &StateCache) {
    let _: StateEntry = cache.value(StateKey::ImageFreeze);
    let _: &'static [StateKey] = StateKey::all();
}

fn main() {
    let _ = inspect_cache;
}
