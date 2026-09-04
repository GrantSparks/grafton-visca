#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    grafton_visca::testing::fuzz::response_parser(bytes);
});
