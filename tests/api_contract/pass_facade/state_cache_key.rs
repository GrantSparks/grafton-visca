// The canonical root cache is an owner-backed target view, not a standalone
// mutable value, so it exists only with a facade. Construction is exercised
// through Session and Camera facade tests; this fixture keeps the root type
// public.
use grafton_visca::{StateCache, StateEntry, StateKey};

fn inspect_cache(cache: &StateCache) {
    let _: StateEntry = cache.value(StateKey::ImageFreeze);
    let _: &'static [StateKey] = StateKey::all();
}

fn main() {
    let _ = inspect_cache;
}
