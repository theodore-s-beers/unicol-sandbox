#![warn(clippy::pedantic)]

use std::cmp::Ordering;
use unicol_sandbox::{
    compare_sort_keys, get_char_values, get_collation_element_array, str_to_sort_key,
    CollationOptions, KeysSource,
};

fn main() {
    //
    // DUCET, NON-IGNORABLE
    //

    let test_data =
        std::fs::read_to_string("test-data/CollationTest_NON_IGNORABLE_SHORT.txt").unwrap();

    let options = CollationOptions {
        keys_source: KeysSource::Ducet,
        shifting: false,
    };

    let start = std::time::Instant::now();

    let mut max_sk: Vec<u16> = Vec::new();

    for line in test_data.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let hex_values: Vec<&str> = line.split(' ').collect();
        let mut test_string = String::new();

        for s in hex_values {
            let val = u32::from_str_radix(s, 16).unwrap();
            // This is BS, but we have to use an unsafe method because the tests deliberately
            // introduce invalid character values.
            let c = unsafe { std::char::from_u32_unchecked(val) };
            test_string.push(c);
        }

        let sk = str_to_sort_key(&test_string, &options);

        let comparison = compare_sort_keys(&sk, &max_sk);
        if comparison == Ordering::Less {
            panic!();
        }

        max_sk = sk;
    }

    let duration = start.elapsed();

    let total_lines = test_data.lines().count();

    println!("Passed CollationTest_NON_IGNORABLE");
    println!("Compared {} lines in {:?}", total_lines, duration);
    println!();

    //
    // DUCET, SHIFTED
    //

    let test_data = std::fs::read_to_string("test-data/CollationTest_SHIFTED_SHORT.txt").unwrap();

    let options = CollationOptions {
        keys_source: KeysSource::Ducet,
        shifting: true,
    };

    let start = std::time::Instant::now();

    let mut max_sk = Vec::new();

    for line in test_data.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let hex_values: Vec<&str> = line.split(' ').collect();
        let mut test_string = String::new();

        for s in hex_values {
            let val = u32::from_str_radix(s, 16).unwrap();
            // This is BS, but we have to use an unsafe method because the tests deliberately
            // introduce invalid character values.
            let c = unsafe { std::char::from_u32_unchecked(val) };
            test_string.push(c);
        }

        let sk = str_to_sort_key(&test_string, &options);

        let comparison = compare_sort_keys(&sk, &max_sk);
        if comparison == Ordering::Less {
            panic!();
        }

        max_sk = sk;
    }

    let duration = start.elapsed();

    let total_lines = test_data.lines().count();

    println!("Passed CollationTest_SHIFTED");
    println!("Compared {} lines in {:?}", total_lines, duration);
    println!();

    //
    // CLDR, NON-IGNORABLE
    //

    println!("Now working on CollationTest_CLDR_NON_IGNORABLE...");
    println!();

    let test_data =
        std::fs::read_to_string("test-data/CollationTest_CLDR_NON_IGNORABLE_SHORT.txt").unwrap();

    let options = CollationOptions {
        keys_source: KeysSource::Cldr,
        shifting: false,
    };

    let mut max_sk = Vec::new();

    for (i, line) in test_data.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let hex_values: Vec<&str> = line.split(' ').collect();
        let mut test_string = String::new();

        for s in hex_values {
            let val = u32::from_str_radix(s, 16).unwrap();
            // This is BS, but we have to use an unsafe method because the tests deliberately
            // introduce invalid character values.
            let c = unsafe { std::char::from_u32_unchecked(val) };
            test_string.push(c);
        }

        let char_values = get_char_values(&test_string);
        let char_values_clone = char_values.clone();
        let cea = get_collation_element_array(char_values, &options);
        let sk = str_to_sort_key(&test_string, &options);

        let comparison = compare_sort_keys(&sk, &max_sk);
        if comparison == Ordering::Less {
            println!("Made it to this line (no. {}) before choking:", i);
            println!("{}", line);
            println!("NFD: {:04X?}", char_values_clone);
            println!("CEA: {:?}", cea);
            println!("SK: {:?}", sk);
            println!("vs. {:?}", max_sk);

            // The problem is again with discontiguous matches. We can find one match separated by
            // two values, or one match separated by one value (checking in that order). This is
            // enough to pass both UCA conformance tests. But the CLDR test wants us to find a set
            // of two matches separated by one value. (Hopefully that's where the madness ends...)
            // This will require a different approach.

            break;
        }

        max_sk = sk;
    }
}
