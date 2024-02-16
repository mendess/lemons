#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    lemonade::parser::parse(data)
        .enumerate()
        .for_each(|(i, _)| {
            if i > 4096 {
                panic!("infinite iterator detected");
            }
        });
});
