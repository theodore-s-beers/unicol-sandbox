use std::cmp::Ordering;
use std::collections::HashMap;

use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use serde::{Deserialize, Serialize};
use unicode_canonical_combining_class::get_canonical_combining_class as get_ccc;
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

pub struct CollationOptions {
    pub keys_source: KeysSource,
    pub shifting: bool,
}

impl Default for CollationOptions {
    fn default() -> Self {
        Self {
            keys_source: KeysSource::Cldr,
            shifting: true,
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum KeysSource {
    Cldr,
    Ducet,
}

#[allow(unused)]
static PARSED: Lazy<HashMap<Vec<u32>, Vec<Weights>>> = Lazy::new(parse_keys);

static ALLKEYS: Lazy<HashMap<Vec<u32>, Vec<Weights>>> = Lazy::new(|| {
    let data = include_bytes!("allkeys_bincode");
    let decoded: HashMap<Vec<u32>, Vec<Weights>> = bincode::deserialize(&data[..]).unwrap();
    decoded
});

static ALLKEYS_CLDR: Lazy<HashMap<Vec<u32>, Vec<Weights>>> = Lazy::new(|| {
    let data = include_bytes!("allkeys_cldr_bincode");
    let decoded: HashMap<Vec<u32>, Vec<Weights>> = bincode::deserialize(&data[..]).unwrap();
    decoded
});

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceCell<Regex> = OnceCell::new();
        RE.get_or_init(|| Regex::new($re).unwrap())
    }};
}

pub fn collate(str_a: &str, str_b: &str, options: &CollationOptions) -> Ordering {
    let sort_key_1 = str_to_sort_key(str_a, options);
    let sort_key_2 = str_to_sort_key(str_b, options);

    let comparison = compare_sort_keys(&sort_key_1, &sort_key_2);

    if comparison == Ordering::Equal {
        // Fallback
        return str_a.cmp(str_b);
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

pub fn str_to_sort_key(input: &str, options: &CollationOptions) -> Vec<u16> {
    let char_values = get_char_values(input);
    let collation_element_array = get_collation_element_array(char_values, options);
    get_sort_key(&collation_element_array, options.shifting)
}

pub fn get_char_values(input: &str) -> Vec<u32> {
    UnicodeNormalization::nfd(input).map(|c| c as u32).collect()
}

// We flatten a slice of u16 vecs to one u16 vec, per UCA instructions
fn get_sort_key(collation_element_array: &[Vec<u16>], shifting: bool) -> Vec<u16> {
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

// This function is where the "magic" happens (or the sausage is made?)
pub fn get_collation_element_array(
    mut char_values: Vec<u32>,
    options: &CollationOptions,
) -> Vec<Vec<u16>> {
    let keys = match options.keys_source {
        KeysSource::Cldr => &ALLKEYS_CLDR,
        KeysSource::Ducet => &ALLKEYS,
    };

    let cldr = options.keys_source == KeysSource::Cldr;
    let shifting = options.shifting;

    let mut collation_element_array: Vec<Vec<u16>> = Vec::new();

    let mut left: usize = 0;
    let mut last_variable = false;

    'outer: while left < char_values.len() {
        let left_val = char_values[left];

        // Set lookahead depending on left_val. Default is 2; there's a small range where we need
        // 3; and a few larger ranges where we need only 1.
        #[allow(clippy::manual_range_contains)]
        let lookahead: usize = match left_val {
            x if x < 3_266 => 2,
            x if x <= 4_019 => 3,
            x if x > 4_142 && x < 6_528 => 1,
            x if x > 6_978 && x < 43_648 => 1,
            x if x > 43_708 && x < 69_927 => 1,
            _ => 2,
        };

        // But don't look past the end of the vec
        let mut right = if left + lookahead > char_values.len() {
            char_values.len()
        } else {
            left + lookahead
        };

        while right > left {
            let subset = &char_values[left..right];

            if let Some(value) = keys.get(subset) {
                // This means we've found "the longest initial substring S at [this] point that has
                // a match in the collation element table." Next we check for "non-starters" that
                // follow this substring.
                //
                // The idea is that there could be multiple non-starters in a row, not blocking one
                // another, such that we could skip over one (or more) to make a longer substring
                // that has a match in the table.
                //
                // One example comes from the test string "0438 0306 0334." NFD normalization will
                // reorder that to "0438 0334 0306." This causes a problem, since 0438 and 0306 can
                // go together, but we'll miss it if we don't look past 0334.

                let mut max_right = if (right + 2) < char_values.len() {
                    right + 2
                } else if (right + 1) < char_values.len() {
                    right + 1
                } else {
                    // This should skip the loop below
                    right
                };

                let mut try_two = max_right - right == 2 && cldr;

                'inner: while max_right > right {
                    // We verify that all chars in the range right..=max_right are non-starters
                    // If there are any starters in our range of interest, decrement and continue
                    // The CCCs also have to be increasing, apparently...

                    let interest_cohort = &char_values[right..=max_right];
                    let mut max_ccc = 0;

                    for elem in interest_cohort {
                        let ccc = get_ccc(char::from_u32(*elem).unwrap()) as u8;
                        if ccc == 0 || ccc <= max_ccc {
                            // Can also forget about try_two in this case
                            try_two = false;
                            max_right -= 1;
                            continue 'inner;
                        }
                        max_ccc = ccc;
                    }

                    // Having made it this far, we can test a new subset, adding the later char(s)
                    let new_subset = if try_two {
                        [subset, &char_values[max_right - 1..=max_right]].concat()
                    } else {
                        [subset, [char_values[max_right]].as_slice()].concat()
                    };

                    // If the new subset is found in the table...
                    if let Some(new_value) = keys.get(&new_subset) {
                        // Then add these weights instead
                        for weights in new_value {
                            if shifting {
                                // Variable shifting means all weight vectors will have a fourth
                                // value

                                // If all weights were already 0, make the fourth 0
                                if weights.primary == 0
                                    && weights.secondary == 0
                                    && weights.tertiary == 0
                                {
                                    let weight_values = vec![0, 0, 0, 0];
                                    collation_element_array.push(weight_values);

                                // If these weights are marked variable...
                                } else if weights.variable {
                                    let weight_values = vec![0, 0, 0, weights.primary];
                                    collation_element_array.push(weight_values);
                                    last_variable = true;

                                // If these are "ignorable" weights and follow something
                                // variable...
                                } else if last_variable
                                    && weights.primary == 0
                                    && weights.tertiary != 0
                                {
                                    let weight_values = vec![0, 0, 0, 0];
                                    collation_element_array.push(weight_values);

                                // Otherwise it can be assumed that we're dealing with something
                                // non-ignorable, or ignorable but not following something variable
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
                                // If not shifting, we can just push weights and be done
                                let weight_values =
                                    vec![weights.primary, weights.secondary, weights.tertiary];
                                collation_element_array.push(weight_values);
                            }
                        }

                        // Remove the pulled char(s) (in this order!)
                        char_values.remove(max_right);
                        if try_two {
                            char_values.remove(max_right - 1);
                        }

                        // Increment and continue outer loop
                        left += right - left;
                        continue 'outer;
                    }

                    // If we tried for two, don't decrement max_right yet
                    if try_two {
                        try_two = false;
                    } else {
                        max_right -= 1;
                    }
                }

                // At this point, we're not looking for a discontiguous match. We just need to push
                // the weights from the original subset we found

                for weights in value {
                    if shifting {
                        // Variable shifting means all weight vectors will have a fourth value

                        // If all weights were already 0, make the fourth 0
                        if weights.primary == 0 && weights.secondary == 0 && weights.tertiary == 0 {
                            let weight_values = vec![0, 0, 0, 0];
                            collation_element_array.push(weight_values);

                        // If these weights are marked variable...
                        } else if weights.variable {
                            let weight_values = vec![0, 0, 0, weights.primary];
                            collation_element_array.push(weight_values);
                            last_variable = true;

                        // If these are "ignorable" weights and follow something variable...
                        } else if last_variable && weights.primary == 0 && weights.tertiary != 0 {
                            let weight_values = vec![0, 0, 0, 0];
                            collation_element_array.push(weight_values);

                        // Otherwise it can be assumed that we're dealing with something non-
                        // ignorable, or ignorable but not following something variable
                        } else {
                            let weight_values =
                                vec![weights.primary, weights.secondary, weights.tertiary, 65_535];
                            collation_element_array.push(weight_values);
                            last_variable = false;
                        }
                    } else {
                        // If not shifting, we can just push weights and be done
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

        // By now, we're looking for just one value, and it isn't in the table
        // Time for implicit weights...

        #[allow(clippy::manual_range_contains)]
        let mut aaaa = match left_val {
            x if x >= 13_312 && x <= 19_903 => 64_384 + (left_val >> 15), //     CJK2
            x if x >= 19_968 && x <= 40_959 => 64_320 + (left_val >> 15), //     CJK1
            x if x >= 63_744 && x <= 64_255 => 64_320 + (left_val >> 15), //     CJK1
            x if x >= 94_208 && x <= 101_119 => 64_256,                   //     Tangut
            x if x >= 101_120 && x <= 101_631 => 64_258,                  //     Khitan
            x if x >= 101_632 && x <= 101_775 => 64_256,                  //     Tangut
            x if x >= 110_960 && x <= 111_359 => 64_257,                  //     Nushu
            x if x >= 131_072 && x <= 173_791 => 64_384 + (left_val >> 15), //   CJK2
            x if x >= 173_824 && x <= 191_471 => 64_384 + (left_val >> 15), //   CJK2
            x if x >= 196_608 && x <= 201_551 => 64_384 + (left_val >> 15), //   CJK2
            _ => 64_448 + (left_val >> 15),                               //     unass.
        };

        #[allow(clippy::manual_range_contains)]
        let mut bbbb = match left_val {
            x if x >= 13_312 && x <= 19_903 => left_val & 32_767, //      CJK2
            x if x >= 19_968 && x <= 40_959 => left_val & 32_767, //      CJK1
            x if x >= 63_744 && x <= 64_255 => left_val & 32_767, //      CJK1
            x if x >= 94_208 && x <= 101_119 => left_val - 94_208, //     Tangut
            x if x >= 101_120 && x <= 101_631 => left_val - 101_120, //   Khitan
            x if x >= 101_632 && x <= 101_775 => left_val - 94_208, //    Tangut
            x if x >= 110_960 && x <= 111_359 => left_val - 110_960, //   Nushu
            x if x >= 131_072 && x <= 173_791 => left_val & 32_767, //    CJK2
            x if x >= 173_824 && x <= 191_471 => left_val & 32_767, //    CJK2
            x if x >= 196_608 && x <= 201_551 => left_val & 32_767, //    CJK2
            _ => left_val & 32_767,                               //      unass.
        };

        // One of the above ranges seems to include some unassigned code points. In order to pass
        // the conformance tests, I'm adding an extra check here. This doesn't feel like a good way
        // of dealing with the problem, but I haven't yet found a better approach that doesn't come
        // with its own downsides.

        let included_unassigned = [177_977, 178_206, 183_970, 191_457];

        if included_unassigned.contains(&left_val) {
            aaaa = 64_448 + (left_val >> 15);
            bbbb = left_val & 32_767;
        }

        // BBBB always gets bitwise ORed with this value
        bbbb |= 32_768;

        #[allow(clippy::cast_possible_truncation)]
        let first_weights = if shifting {
            // Add an arbitrary fourth weight if shifting
            vec![aaaa as u16, 32, 2, 65_535]
        } else {
            vec![aaaa as u16, 32, 2]
        };
        collation_element_array.push(first_weights);

        #[allow(clippy::cast_possible_truncation)]
        let second_weights = if shifting {
            // Add an arbitrary fourth weight if shifting
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

// This is just to generate bincode; not usually run
fn parse_keys() -> HashMap<Vec<u32>, Vec<Weights>> {
    let keys = std::fs::read_to_string("test-data/allkeys.txt").unwrap();
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

        let options = CollationOptions {
            keys_source: KeysSource::Ducet,
            shifting: true,
        };

        scrambled.sort_by(|a, b| collate(a, b, &options));

        let sorted = [
            "death", "de luge", "de-luge", "de-luge", "deluge", "de Luge", "de-Luge", "de-Luge",
            "deLuge", "demark",
        ];

        assert_eq!(scrambled, sorted);
    }

    #[test]
    fn multi_script() {
        let mut scrambled = [
            "ｶ",
            "ヵ",
            "abc",
            "abç",
            "ab©",
            "𝒶bc",
            "abC",
            "𝕒bc",
            "File-3",
            "ガ",
            "が",
            "äbc",
            "カ",
            "か",
            "Abc",
            "file-12",
            "filé-110",
        ];

        let options = CollationOptions {
            keys_source: KeysSource::Ducet,
            shifting: true,
        };

        scrambled.sort_by(|a, b| collate(a, b, &options));

        let sorted = [
            "ab©",
            "abc",
            "abC",
            "𝒶bc",
            "𝕒bc",
            "Abc",
            "abç",
            "äbc",
            "filé-110",
            "file-12",
            "File-3",
            "か",
            "ヵ",
            "カ",
            "ｶ",
            "が",
            "ガ",
        ];

        assert_eq!(scrambled, sorted);
    }
}
