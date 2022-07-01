#![warn(clippy::pedantic)]

use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use unicode_canonical_combining_class::{get_canonical_combining_class, CanonicalCombiningClass};
use unicode_normalization::char::is_public_assigned;
use unicode_normalization::UnicodeNormalization;

#[derive(Deserialize, Serialize)]
struct Weights {
    variable: bool,
    primary: u16,
    secondary: u16,
    tertiary: u16,
}

impl Weights {
    fn new() -> Self {
        Self {
            variable: false,
            primary: 0,
            secondary: 0,
            tertiary: 0,
        }
    }
}

#[allow(unused)]
static PARSED: Lazy<HashMap<Vec<u32>, Vec<Weights>>> = Lazy::new(parse_keys);
static PARSED_BIN: Lazy<HashMap<Vec<u32>, Vec<Weights>>> = Lazy::new(|| {
    let data = std::fs::read("test-data/allkeys_bincode").unwrap();
    let decoded: HashMap<Vec<u32>, Vec<Weights>> = bincode::deserialize(&data[..]).unwrap();
    decoded
});

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceCell<Regex> = OnceCell::new();
        RE.get_or_init(|| Regex::new($re).unwrap())
    }};
}

fn main() {
    let test_data =
        std::fs::read_to_string("test-data/CollationTest_NON_IGNORABLE_SHORT.txt").unwrap();

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
            let c = unsafe { std::char::from_u32_unchecked(val) };
            test_string.push(c);
        }

        let char_values = get_char_values(&test_string);
        let cea = get_collation_element_array(char_values);
        let sk = get_sort_key(&cea);

        let comparison = compare_sort_keys(&sk, &max_sk);
        if comparison == Ordering::Less {
            panic!();
        }

        max_sk = sk;
    }

    let duration = start.elapsed();

    let total_lines = test_data.lines().count();

    println!("Passed CollationTest_NON_IGNORABLE");
    println!("(with a few caveats...)");
    println!("Compared {} lines in {:?}", total_lines, duration);
}

fn _collate(str_1: &str, str_2: &str) -> Ordering {
    let sort_key_1 = _str_to_sort_key(str_1);
    let sort_key_2 = _str_to_sort_key(str_2);

    let comparison = compare_sort_keys(&sort_key_1, &sort_key_2);

    if comparison == Ordering::Equal {
        // Fallback
        return str_1.cmp(str_2);
    }

    comparison
}

fn compare_sort_keys(a: &[u16], b: &[u16]) -> Ordering {
    let min_sort_key_length = a.len().min(b.len());

    for i in 0..min_sort_key_length {
        if a[i] < b[i] {
            return Ordering::Less;
        }

        if a[i] > b[i] {
            return Ordering::Greater;
        }
    }

    Ordering::Equal
}

fn _str_to_sort_key(input: &str) -> Vec<u16> {
    let char_values = get_char_values(input);
    let collation_element_array = get_collation_element_array(char_values);
    get_sort_key(&collation_element_array)
}

fn get_char_values(input: &str) -> Vec<u32> {
    UnicodeNormalization::nfd(input)
        .into_iter()
        .map(|c| c as u32)
        .collect()
}

// This function is where the "magic" happens (or the sausage is made?)
fn get_collation_element_array(mut char_values: Vec<u32>) -> Vec<Vec<u16>> {
    let mut collation_element_array: Vec<Vec<u16>> = Vec::new();
    let mut left: usize = 0;

    'outer: while left < char_values.len() {
        // If left <= 3200 (0C80), only need to look one ahead
        #[allow(clippy::match_on_vec_items)]
        let lookahead: usize = match char_values[left] {
            x if x <= 3200 => 2,
            _ => 3,
        };

        // But don't look past the end of the vec
        let mut right = if left + lookahead > char_values.len() {
            char_values.len()
        } else {
            left + lookahead
        };

        while right > left {
            let subset = &char_values[left..right];

            if let Some(value) = PARSED_BIN.get(subset) {
                // This means we've found "the longest initial substring S at [this] point that has
                // a match in the collation element table."
                //
                // We should next check for "non-starters" that follow this substring.
                //
                // The idea is that there could be multiple non-starters in a row, not blocking one
                // another, such that we could skip over one (or more) to make a longer substring
                // that has a match in the table.
                //
                // The first example that we need to check is from the test string "0438 0306
                // 0334." NFD normalization will reorder that to "0438 0334 0306." This causes a
                // problem, since 0438 and 0306 can go together, but we'll miss it if we don't skip
                // over 0334 in the normalized version. Both 0306 and 0334 are non-blocking (if
                // I've understood correctly).

                let skip_index = right + 1;
                // If there's enough space left in the slice...
                if skip_index < char_values.len() {
                    let next_char = char::from_u32(char_values[right]).unwrap();
                    let next_ccc = get_canonical_combining_class(next_char);

                    // If the next char (not the skip) is a non-starter...
                    if next_ccc != CanonicalCombiningClass::NotReordered {
                        let skip_char = char::from_u32(char_values[skip_index]).unwrap();
                        let skip_ccc = get_canonical_combining_class(skip_char);

                        // If the skip char is also a non-starter, and not blocked...
                        if skip_ccc != CanonicalCombiningClass::NotReordered && next_ccc < skip_ccc
                        {
                            let new_subset =
                                [subset, [char_values[skip_index]].as_slice()].concat();

                            // If the new substring is found in the table...
                            if let Some(new_value) = PARSED_BIN.get(&new_subset) {
                                // Then add these weights instead
                                for weights in new_value {
                                    let weight_values =
                                        vec![weights.primary, weights.secondary, weights.tertiary];
                                    collation_element_array.push(weight_values);
                                }

                                // Remove the skip char
                                char_values.remove(skip_index);

                                // Increment and continue outer loop
                                left += right - left;
                                continue 'outer;
                            }
                        }
                    }
                }

                // Otherwise, add the weights of the original subset we found
                for weights in value {
                    let weight_values = vec![weights.primary, weights.secondary, weights.tertiary];
                    collation_element_array.push(weight_values);
                }

                // Increment and continue outer loop
                left += right - left;
                continue 'outer;
            }

            // Shorten slice to try again
            right -= 1;
        }

        // At this point, we're looking for one value, and it isn't in the table
        // Time for implicit weights...

        let problem_val = char_values[left];
        let problem_char = unsafe { char::from_u32_unchecked(problem_val) };

        #[allow(clippy::manual_range_contains)]
        let mut aaaa = match problem_val {
            x if x >= 13_312 && x <= 19_903 => (64_384 + (problem_val >> 15)), //   CJK2
            x if x >= 19_968 && x <= 40_959 => (64_320 + (problem_val >> 15)), //   CJK1
            x if x >= 63_744 && x <= 64_255 => (64_320 + (problem_val >> 15)), //   CJK1
            x if x >= 94_208 && x <= 101_119 => 64_256,                        //   Tangut
            x if x >= 101_120 && x <= 101_631 => 64_258,                       //   Khitan
            x if x >= 101_632 && x <= 101_775 => 64_256,                       //   Tangut
            x if x >= 110_960 && x <= 111_359 => 64_257,                       //   Nushu
            x if x >= 131_072 && x <= 173_791 => (64_384 + (problem_val >> 15)), // CJK2
            x if x >= 173_824 && x <= 191_471 => (64_384 + (problem_val >> 15)), // CJK2
            x if x >= 196_608 && x <= 201_551 => (64_384 + (problem_val >> 15)), // CJK2
            _ => (64_448 + (problem_val >> 15)),                               //   unass.
        };

        #[allow(clippy::manual_range_contains)]
        let mut bbbb = match problem_val {
            x if x >= 13_312 && x <= 19_903 => (problem_val & 32_767), //     CJK2
            x if x >= 19_968 && x <= 40_959 => (problem_val & 32_767), //     CJK1
            x if x >= 63_744 && x <= 64_255 => (problem_val & 32_767), //     CJK1
            x if x >= 94_208 && x <= 101_119 => (problem_val - 94_208), //    Tangut
            x if x >= 101_120 && x <= 101_631 => (problem_val - 101_120), //  Khitan
            x if x >= 101_632 && x <= 101_775 => (problem_val - 94_208), //   Tangut
            x if x >= 110_960 && x <= 111_359 => (problem_val - 110_960), //  Nushu
            x if x >= 131_072 && x <= 173_791 => (problem_val & 32_767), //   CJK2
            x if x >= 173_824 && x <= 191_471 => (problem_val & 32_767), //   CJK2
            x if x >= 196_608 && x <= 201_551 => (problem_val & 32_767), //   CJK2
            _ => (problem_val & 32_767),                               //     unass.
        };

        // Some of the above is incorrect. Need to check for individual unassigned characters
        // Or I could fix the above match statements, maybe...
        if !is_public_assigned(problem_char) {
            aaaa = 64_448 + (problem_val >> 15);
            bbbb = problem_val & 32_767;
        }

        // BBBB always gets bitwise ORed with this value
        bbbb |= 32_768;

        #[allow(clippy::cast_possible_truncation)]
        let first_weights = vec![aaaa as u16, 32, 2];
        collation_element_array.push(first_weights);

        #[allow(clippy::cast_possible_truncation)]
        let second_weights = vec![bbbb as u16, 0, 0];
        collation_element_array.push(second_weights);

        // Finally, increment and let outer loop continue
        left += 1;
    }

    collation_element_array
}

// We flatten a slice of u16 vecs to one u16 vec, according to UCA rules
fn get_sort_key(collation_element_array: &[Vec<u16>]) -> Vec<u16> {
    let mut sort_key: Vec<u16> = Vec::new();

    for i in 0..3 {
        if i > 0 {
            sort_key.push(0);
        }

        for elem in collation_element_array.iter() {
            if elem[i] != 0 {
                sort_key.push(elem[i]);
            }
        }
    }

    sort_key
}

// This is just to generate bincode; not usually run
fn parse_keys() -> HashMap<Vec<u32>, Vec<Weights>> {
    let keys = std::fs::read_to_string("test-data/allkeys_CLDR.txt").unwrap();
    let mut map = HashMap::new();

    for line in keys.lines() {
        if line.is_empty() || line.starts_with('@') || line.starts_with('#') {
            continue;
        }

        let mut split_at_semicolon = line.split(';');
        let left_of_semicolon = split_at_semicolon.next().unwrap();
        let right_of_semicolon = split_at_semicolon.next().unwrap();
        let left_of_hash = right_of_semicolon.split('#').next().unwrap();

        let mut k: Vec<u32> = Vec::new();
        let re_key = regex!(r"[\dA-F]{4,5}");
        for cap in re_key.captures_iter(left_of_semicolon) {
            let as_u32 = u32::from_str_radix(&cap[0], 16).unwrap();
            k.push(as_u32);
        }

        let mut v: Vec<Weights> = Vec::new();
        let re_weights = regex!(r"[*.\dA-F]{15}");
        let re_value = regex!(r"[\dA-F]{4}");

        for cap in re_weights.captures_iter(left_of_hash) {
            let weights_str = &cap[0];
            let mut weights: Weights = Weights::new();

            if weights_str.contains('*') {
                weights.variable = true;
            }

            let mut vals = re_value.captures_iter(weights_str);
            weights.primary = u16::from_str_radix(&vals.next().unwrap()[0], 16).unwrap();
            weights.secondary = u16::from_str_radix(&vals.next().unwrap()[0], 16).unwrap();
            weights.tertiary = u16::from_str_radix(&vals.next().unwrap()[0], 16).unwrap();

            v.push(weights);
        }

        map.insert(k, v);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("byte_dump", bytes).unwrap();

    map
}
