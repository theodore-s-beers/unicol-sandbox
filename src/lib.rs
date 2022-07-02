use std::cmp::Ordering;
use std::collections::HashMap;

use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use serde::{Deserialize, Serialize};
use unicode_canonical_combining_class::{get_canonical_combining_class, CanonicalCombiningClass};
use unicode_normalization::char::is_public_assigned;
use unicode_normalization::UnicodeNormalization;

#[derive(Deserialize, Serialize)]
pub struct Weights {
    pub variable: bool,
    pub primary: u16,
    pub secondary: u16,
    pub tertiary: u16,
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

pub static PARSED_BIN: Lazy<HashMap<Vec<u32>, Vec<Weights>>> = Lazy::new(|| {
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

pub fn collate(str_1: &str, str_2: &str, shifting: bool) -> Ordering {
    let sort_key_1 = str_to_sort_key(str_1, shifting);
    let sort_key_2 = str_to_sort_key(str_2, shifting);

    let comparison = compare_sort_keys(&sort_key_1, &sort_key_2);

    if comparison == Ordering::Equal {
        // Fallback
        return str_1.cmp(str_2);
    }

    comparison
}

pub fn compare_sort_keys(a: &[u16], b: &[u16]) -> Ordering {
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

fn str_to_sort_key(input: &str, shifting: bool) -> Vec<u16> {
    let char_values = get_char_values(input);
    let collation_element_array = get_collation_element_array(char_values, shifting);
    get_sort_key(&collation_element_array, shifting)
}

pub fn get_char_values(input: &str) -> Vec<u32> {
    UnicodeNormalization::nfd(input)
        .into_iter()
        .map(|c| c as u32)
        .collect()
}

// This function is where the "magic" happens (or the sausage is made?)
pub fn get_collation_element_array(mut char_values: Vec<u32>, shifting: bool) -> Vec<Vec<u16>> {
    let mut collation_element_array: Vec<Vec<u16>> = Vec::new();

    let mut left: usize = 0;
    let mut last_variable = false;

    'outer: while left < char_values.len() {
        // If left <= 3200 (0C80), only need to look one ahead
        let lookahead: usize = if char_values[left] <= 3_200 { 2 } else { 3 };

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

                let mut skip_index = if (right + 2) < char_values.len() {
                    right + 2
                } else if (right + 1) < char_values.len() {
                    right + 1
                } else {
                    // This should skip all the stuff below
                    right
                };

                'inner: while skip_index > right {
                    // We verify that all chars in the range right..skip_index are non-starters
                    // If there are any starters in our range of interest, decrement and continue
                    let interest_cohort = &char_values[right..skip_index];
                    for elem in interest_cohort {
                        let c = char::from_u32(*elem).unwrap();
                        if get_canonical_combining_class(c) == CanonicalCombiningClass::NotReordered
                        {
                            skip_index -= 1;
                            continue 'inner;
                        }
                    }

                    let left_of_skip = char::from_u32(char_values[skip_index - 1]).unwrap();
                    let skip_char = char::from_u32(char_values[skip_index]).unwrap();

                    let left_ccc = get_canonical_combining_class(left_of_skip);
                    let skip_ccc = get_canonical_combining_class(skip_char);

                    // If skip char is starting, or relationship between skip char and the one
                    // preceding it is bad, decrement skip index and continue
                    if skip_ccc == CanonicalCombiningClass::NotReordered || skip_ccc <= left_ccc {
                        skip_index -= 1;
                        continue;
                    }

                    let new_subset = [subset, [char_values[skip_index]].as_slice()].concat();

                    // If the new subset is found in the table...
                    if let Some(new_value) = PARSED_BIN.get(&new_subset) {
                        // Then add these weights instead
                        for weights in new_value {
                            if shifting {
                                // All weight vectors will have a fourth value added
                                if weights.primary == 0
                                    && weights.secondary == 0
                                    && weights.tertiary == 0
                                {
                                    let weight_values = vec![0, 0, 0, 0];
                                    collation_element_array.push(weight_values);
                                } else if weights.variable {
                                    let weight_values = vec![0, 0, 0, weights.primary];
                                    collation_element_array.push(weight_values);
                                    last_variable = true;
                                } else if last_variable
                                    && weights.primary == 0
                                    && weights.tertiary != 0
                                {
                                    let weight_values = vec![0, 0, 0, 0];
                                    collation_element_array.push(weight_values);
                                } else {
                                    let weight_values = vec![
                                        weights.primary,
                                        weights.secondary,
                                        weights.tertiary,
                                        65_535,
                                    ];
                                    collation_element_array.push(weight_values);
                                    last_variable = false;
                                }
                            } else {
                                // Do normal shit
                                let weight_values =
                                    vec![weights.primary, weights.secondary, weights.tertiary];
                                collation_element_array.push(weight_values);
                            }
                        }

                        // Remove the skip char
                        char_values.remove(skip_index);

                        // Increment and continue outer loop
                        left += right - left;
                        continue 'outer;
                    }

                    skip_index -= 1;
                }

                // Not variable, primary weight non-zero: add fourth weight of 65_535; set
                // last_variable to false
                //
                // Not following variable, primary weight zero, tertiary weight non-zero: add fourth
                // weight of 65_535; set last_variable to false
                //
                // Variable: first three weights zeroed; fourth weight is former primary; set
                // last_variable to true
                //
                // Following variable, primary weight zero, tertiary weight non-zero: all four
                // weights zeroed; set last_variable to false
                //
                // All three weights already zero: add 0 fourth weight; set last_variable to false

                // Otherwise, add the weights of the original subset we found
                for weights in value {
                    if shifting {
                        // All weight vectors will have a fourth value added
                        if weights.primary == 0 && weights.secondary == 0 && weights.tertiary == 0 {
                            let weight_values = vec![0, 0, 0, 0];
                            collation_element_array.push(weight_values);
                        } else if weights.variable {
                            let weight_values = vec![0, 0, 0, weights.primary];
                            collation_element_array.push(weight_values);
                            last_variable = true;
                        } else if last_variable && weights.primary == 0 && weights.tertiary != 0 {
                            let weight_values = vec![0, 0, 0, 0];
                            collation_element_array.push(weight_values);
                        } else {
                            let weight_values =
                                vec![weights.primary, weights.secondary, weights.tertiary, 65_535];
                            collation_element_array.push(weight_values);
                            last_variable = false;
                        }
                    } else {
                        // Do normal shit
                        let weight_values =
                            vec![weights.primary, weights.secondary, weights.tertiary];
                        collation_element_array.push(weight_values);
                    }
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
        let first_weights = if shifting {
            vec![aaaa as u16, 32, 2, 65_535]
        } else {
            vec![aaaa as u16, 32, 2]
        };
        collation_element_array.push(first_weights);

        #[allow(clippy::cast_possible_truncation)]
        let second_weights = if shifting {
            vec![bbbb as u16, 0, 0, 65_535]
        } else {
            vec![bbbb as u16, 0, 0]
        };
        collation_element_array.push(second_weights);

        // Finally, increment and let outer loop continue
        left += 1;
    }

    collation_element_array
}

// We flatten a slice of u16 vecs to one u16 vec, according to UCA rules
pub fn get_sort_key(collation_element_array: &[Vec<u16>], shifting: bool) -> Vec<u16> {
    let max_level = if shifting { 4 } else { 3 };
    let mut sort_key: Vec<u16> = Vec::new();

    for i in 0..max_level {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deluge_shifted() {
        let mut scrambled = [
            "demark", "de-luge", "deluge", "de-Luge", "de luge", "de-luge", "deLuge", "de Luge",
            "de-Luge", "death",
        ];

        scrambled.sort_by(|a, b| collate(a, b, true));

        let sorted = [
            "death", "de luge", "de-luge", "de-luge", "deluge", "de Luge", "de-Luge", "de-Luge",
            "deLuge", "demark",
        ];

        assert_eq!(scrambled, sorted);
    }
}
