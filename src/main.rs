#![warn(clippy::pedantic)]

use std::cmp::Ordering;
use unicol_sandbox::{collate_no_tiebreak, CollationOptions, KeysSource};

fn main() {
    //
    // DUCET, NON-IGNORABLE
    //

    let path = "test-data/CollationTest_NON_IGNORABLE_SHORT.txt";

    let options = CollationOptions {
        keys_source: KeysSource::Ducet,
        shifting: false,
    };

    conformance(path, options);

    println!("Passed CollationTest_NON_IGNORABLE");

    //
    // DUCET, SHIFTED
    //

    let path = "test-data/CollationTest_SHIFTED_SHORT.txt";

    let options = CollationOptions {
        keys_source: KeysSource::Ducet,
        shifting: true,
    };

    conformance(path, options);

    println!("Passed CollationTest_SHIFTED");

    //
    // CLDR, NON-IGNORABLE
    //

    let path = "test-data/CollationTest_CLDR_NON_IGNORABLE_SHORT.txt";

    let options = CollationOptions {
        keys_source: KeysSource::Cldr,
        shifting: false,
    };

    conformance(path, options);

    println!("Passed CollationTest_CLDR_NON_IGNORABLE");

    //
    // CLDR, SHIFTED
    //

    let path = "test-data/CollationTest_CLDR_SHIFTED_SHORT.txt";

    let options = CollationOptions {
        keys_source: KeysSource::Cldr,
        shifting: true,
    };

    conformance(path, options);

    println!("Passed CollationTest_CLDR_SHIFTED");
}

fn conformance(path: &str, options: CollationOptions) {
    let test_data = std::fs::read_to_string(path).unwrap();

    let mut max_line = String::new();

    for line in test_data.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let hex_values: Vec<&str> = line.split(' ').collect();
        let mut test_string = String::new();

        for s in hex_values {
            let val = u32::from_str_radix(s, 16).unwrap();
            // This is BS, but we have to use an unsafe method because the tests deliberately
            // introduce invalid character values
            let c = unsafe { std::char::from_u32_unchecked(val) };
            test_string.push(c);
        }

        let comparison = collate_no_tiebreak(&test_string, &max_line, options);
        if comparison == Ordering::Less {
            panic!();
        }

        max_line = test_string;
    }
}
