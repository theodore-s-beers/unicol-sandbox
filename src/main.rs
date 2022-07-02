#![warn(clippy::pedantic)]

use std::cmp::Ordering;
use unicol_sandbox::{
    compare_sort_keys, get_char_values, get_collation_element_array, get_sort_key,
};

fn main() {
    let test_data =
        std::fs::read_to_string("test-data/CollationTest_NON_IGNORABLE_SHORT.txt").unwrap();

    let start = std::time::Instant::now();

    let shifting = false;
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

        let char_values = get_char_values(&test_string);
        let cea = get_collation_element_array(char_values, shifting);
        let sk = get_sort_key(&cea, shifting);

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

    let second_test_data =
        std::fs::read_to_string("test-data/CollationTest_SHIFTED_SHORT.txt").unwrap();

    let start = std::time::Instant::now();

    let shifting = true;
    // let mut max_line = String::new();
    // let mut max_test_string = String::new();
    // let mut max_char_values: Vec<u32> = Vec::new();
    // let mut max_cea: Vec<Vec<u16>> = Vec::new();
    let mut max_sk = Vec::new();

    let mut ignored: u8 = 0;

    for (i, line) in second_test_data.lines().enumerate() {
        if i > 7 && line.starts_with('#') {
            ignored += 1;
        }

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
        // let char_values_clone = char_values.clone();
        let cea = get_collation_element_array(char_values, shifting);
        let sk = get_sort_key(&cea, shifting);

        let comparison = compare_sort_keys(&sk, &max_sk);
        if comparison == Ordering::Less {
            // println!("Made it to line {} before choking", i);
            // println!();

            // println!("Last passing line:");
            // println!("{}", max_line);
            // println!("“{}”", max_test_string);
            // println!("NFD chars: {:04X?}", max_char_values);
            // println!("CEA: {:X?}", max_cea);
            // println!("SK: {:X?}", max_sk);
            // println!();

            // println!("Failing line:");
            // println!("{}", line);
            // println!("“{}”", test_string);
            // println!("NFD chars: {:04X?}", char_values_clone);
            // println!("CEA: {:X?}", cea);
            // println!("SK: {:X?}", sk);

            // errors += 1;
            panic!();
        }

        // max_line = line.into();
        // max_test_string = test_string;
        // max_char_values = char_values_clone;
        // max_cea = cea;
        max_sk = sk;
    }

    let duration = start.elapsed();

    let total_lines = second_test_data.lines().count();

    println!("Passed CollationTest_SHIFTED");
    println!("(except that {} lines were ignored)", ignored);
    println!("Compared {} lines in {:?}", total_lines, duration);
}
