#![warn(clippy::pedantic)]

use std::cmp::Ordering;
use unicol_sandbox::{compare_sort_keys, str_to_sort_key, CollationOptions, KeysSource};

#[allow(clippy::too_many_lines)]
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

    println!("Passed CollationTest_NON_IGNORABLE");

    //
    // DUCET, SHIFTED
    //

    let test_data = std::fs::read_to_string("test-data/CollationTest_SHIFTED_SHORT.txt").unwrap();

    let options = CollationOptions {
        keys_source: KeysSource::Ducet,
        shifting: true,
    };

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

    println!("Passed CollationTest_SHIFTED");

    //
    // CLDR, NON-IGNORABLE
    //

    let test_data =
        std::fs::read_to_string("test-data/CollationTest_CLDR_NON_IGNORABLE_SHORT.txt").unwrap();

    let options = CollationOptions {
        keys_source: KeysSource::Cldr,
        shifting: false,
    };

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

    println!("Passed CollationTest_CLDR_NON_IGNORABLE");

    //
    // CLDR, SHIFTED
    //

    let test_data =
        std::fs::read_to_string("test-data/CollationTest_CLDR_SHIFTED_SHORT.txt").unwrap();

    let options = CollationOptions {
        keys_source: KeysSource::Cldr,
        shifting: true,
    };

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

    println!("Passed CollationTest_CLDR_SHIFTED");
}
