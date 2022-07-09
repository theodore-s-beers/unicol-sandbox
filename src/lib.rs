use std::cmp::Ordering;
use std::collections::HashMap;

use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use serde::{Deserialize, Serialize};
use unicode_canonical_combining_class::get_canonical_combining_class as get_ccc;
use unicode_normalization::UnicodeNormalization;

//
// Structs etc.
//

#[derive(
    Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, Deserialize, Serialize,
)]
pub struct Weights {
    variable: bool,
    primary: u16,
    secondary: u16,
    tertiary: u16,
}

impl Weights {
    fn new() -> Self {
        Default::default()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
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

#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Debug)]
pub enum KeysSource {
    Cldr,
    Ducet,
}

//
// Static/const
//

pub static PARSED: Lazy<HashMap<Vec<u32>, Vec<Weights>>> = Lazy::new(parse_keys);

static FCD: Lazy<HashMap<u32, u16>> = Lazy::new(|| {
    let data = include_bytes!("bincode/fcd");
    let decoded: HashMap<u32, u16> = bincode::deserialize(data).unwrap();
    decoded
});

static SING: Lazy<HashMap<u32, Vec<Weights>>> = Lazy::new(|| {
    let data = include_bytes!("bincode/singles");
    let decoded: HashMap<u32, Vec<Weights>> = bincode::deserialize(data).unwrap();
    decoded
});

static MULT: Lazy<HashMap<Vec<u32>, Vec<Weights>>> = Lazy::new(|| {
    let data = include_bytes!("bincode/multis");
    let decoded: HashMap<Vec<u32>, Vec<Weights>> = bincode::deserialize(data).unwrap();
    decoded
});

static SING_CLDR: Lazy<HashMap<u32, Vec<Weights>>> = Lazy::new(|| {
    let data = include_bytes!("bincode/singles_cldr");
    let decoded: HashMap<u32, Vec<Weights>> = bincode::deserialize(data).unwrap();
    decoded
});

static MULT_CLDR: Lazy<HashMap<Vec<u32>, Vec<Weights>>> = Lazy::new(|| {
    let data = include_bytes!("bincode/multis_cldr");
    let decoded: HashMap<Vec<u32>, Vec<Weights>> = bincode::deserialize(data).unwrap();
    decoded
});

const NEED_THREE: [u32; 4] = [3_270, 3_545, 4_018, 4_019];

const NEED_TWO: [u32; 59] = [
    76, 108, 1_048, 1_080, 1_575, 1_608, 1_610, 2_503, 2_887, 2_962, 3_014, 3_015, 3_142, 3_263,
    3_274, 3_398, 3_399, 3_548, 3_648, 3_649, 3_650, 3_651, 3_652, 3_661, 3_776, 3_777, 3_778,
    3_779, 3_780, 3_789, 3_953, 4_133, 6_581, 6_582, 6_583, 6_586, 6_917, 6_919, 6_921, 6_923,
    6_925, 6_929, 6_970, 6_972, 6_974, 6_975, 6_978, 43_701, 43_702, 43_705, 43_707, 43_708,
    69_937, 69_938, 70_471, 70_841, 71_096, 71_097, 71_989,
];

const INCLUDED_UNASSIGNED: [u32; 4] = [177_977, 178_206, 183_970, 191_457];

//
// Macros
//

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceCell<Regex> = OnceCell::new();
        RE.get_or_init(|| Regex::new($re).unwrap())
    }};
}

//
// Functions, public
//

pub fn collate(str_a: &str, str_b: &str, options: CollationOptions) -> Ordering {
    let a_nfd = get_nfd(str_a);
    let b_nfd = get_nfd(str_b);

    if a_nfd == b_nfd {
        return Ordering::Equal;
    }

    let a_sk = nfd_to_sk(a_nfd, options);
    let b_sk = nfd_to_sk(b_nfd, options);

    let comparison = compare_sort_keys(&a_sk, &b_sk);

    if comparison == Ordering::Equal {
        // Tiebreaker
        return str_a.cmp(str_b);
    }

    comparison
}

pub fn collate_no_tiebreak(str_a: &str, str_b: &str, options: CollationOptions) -> Ordering {
    if str_a == str_b {
        return Ordering::Equal;
    }

    let mut a_nfd = get_nfd(str_a);
    let mut b_nfd = get_nfd(str_b);

    if a_nfd == b_nfd {
        return Ordering::Equal;
    }

    let cldr = options.keys_source == KeysSource::Cldr;
    trim_prefix(&mut a_nfd, &mut b_nfd, cldr);

    let a_sk = nfd_to_sk(a_nfd, options);
    let b_sk = nfd_to_sk(b_nfd, options);

    compare_sort_keys(&a_sk, &b_sk)
}

fn trim_prefix(a: &mut Vec<u32>, b: &mut Vec<u32>, cldr: bool) {
    let prefix_len = find_prefix(a, b);

    if prefix_len > 0 {
        for elem in &a[0..prefix_len] {
            if NEED_THREE.contains(elem) || NEED_TWO.contains(elem) {
                return;
            }
        }

        let sing = if cldr { &SING_CLDR } else { &SING };

        if let Some(row) = sing.get(&a[prefix_len - 1]) {
            for weights in row {
                if weights.variable || weights.primary == 0 {
                    return;
                }
            }
        }

        a.drain(0..prefix_len);
        b.drain(0..prefix_len);
    }
}

fn find_prefix(a: &[u32], b: &[u32]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
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

fn nfd_to_sk(input: Vec<u32>, options: CollationOptions) -> Vec<u16> {
    let collation_element_array = get_collation_element_array(input, options);
    get_sort_key(&collation_element_array, options.shifting)
}

fn get_nfd(input: &str) -> Vec<u32> {
    if fcd(input) {
        input.chars().map(|c| c as u32).collect()
    } else {
        UnicodeNormalization::nfd(input).map(|c| c as u32).collect()
    }
}

//
// Functions, private
//

fn fcd(input: &str) -> bool {
    let mut c_as_u32: u32;
    let mut curr_lead_cc: u8;
    let mut curr_trail_cc: u8;

    let mut prev_trail_cc: u8 = 0;

    for c in input.chars() {
        c_as_u32 = c as u32;

        if c_as_u32 < 192 {
            prev_trail_cc = 0;
            continue;
        }

        if c_as_u32 == 3_969 || (44_032..=55_215).contains(&c_as_u32) {
            return false;
        }

        if let Some(vals) = FCD.get(&c_as_u32) {
            [curr_lead_cc, curr_trail_cc] = vals.to_be_bytes();
        } else {
            curr_lead_cc = get_ccc(c) as u8;
            curr_trail_cc = get_ccc(c) as u8;
        }

        if curr_lead_cc != 0 && curr_lead_cc < prev_trail_cc {
            return false;
        }

        prev_trail_cc = curr_trail_cc;
    }

    true
}

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

fn get_collation_element_array(mut char_vals: Vec<u32>, opt: CollationOptions) -> Vec<Vec<u16>> {
    let mut cea: Vec<Vec<u16>> = Vec::new();

    let cldr = opt.keys_source == KeysSource::Cldr;
    let shifting = opt.shifting;

    let singles = if cldr { &SING_CLDR } else { &SING };
    let multis = if cldr { &MULT_CLDR } else { &MULT };

    let mut left: usize = 0;
    let mut last_variable = false;

    'outer: while left < char_vals.len() {
        let left_val = char_vals[left];

        // Set lookahead depending on left_val. We need 3 in a few cases; 2 in several dozen cases;
        // and 1 otherwise.
        let lookahead: usize = match left_val {
            x if NEED_THREE.contains(&x) => 3,
            x if NEED_TWO.contains(&x) => 2,
            _ => 1,
        };

        let check_multi = lookahead > 1 && char_vals.len() - left > 1;

        // If lookahead is 1, or if this is the last item in the vec, take an easy path
        if !check_multi {
            // Did we find it? Sure hope so
            if let Some(row) = singles.get(&left_val) {
                // Push weights to collation element array
                for weights in row {
                    if shifting {
                        let weight_values = get_weights_shifting(weights, last_variable);
                        cea.push(weight_values);
                        if weights.variable {
                            last_variable = true;
                        } else if weights.primary != 0 {
                            last_variable = false;
                        }
                    } else {
                        let weight_values =
                            vec![weights.primary, weights.secondary, weights.tertiary];
                        cea.push(weight_values);
                    }
                }

                // Increment and continue outer loop
                left += 1;
                continue 'outer;
            }
        }

        // Next consider multis, if applicable
        // If we just tried to find a single, and didn't find it, we should skip all the way down
        // to the implicit weights section

        // But don't look past end of the vec
        let mut right = if left + lookahead > char_vals.len() {
            char_vals.len()
        } else {
            left + lookahead
        };

        'middle: while check_multi && right > left {
            // If right - left == 1 (which cannot have been the case in the first iteration),
            // attempts to find a slice have failed. So look for one code point, in the singles map
            if right - left == 1 {
                // If we found it, we do still need to check for discontiguous matches
                if let Some(value) = singles.get(&left_val) {
                    // Determine how much further right to look
                    let mut max_right = if right + 2 < char_vals.len() {
                        right + 2
                    } else if right + 1 < char_vals.len() {
                        right + 1
                    } else {
                        // This should skip the loop below. There will be no discontiguous match
                        right
                    };

                    let mut try_two = max_right - right == 2 && cldr;

                    'inner: while max_right > right {
                        // Make sure the sequence of CCC values is kosher
                        let interest_cohort = &char_vals[right..=max_right];
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
                            [[left_val].as_slice(), &char_vals[max_right - 1..=max_right]].concat()
                        } else {
                            vec![left_val, char_vals[max_right]]
                        };

                        // If the new subset is found in the table...
                        if let Some(new_value) = multis.get(&new_subset) {
                            // Then add these weights instead
                            for weights in new_value {
                                if shifting {
                                    let weight_values =
                                        get_weights_shifting(weights, last_variable);
                                    cea.push(weight_values);
                                    if weights.variable {
                                        last_variable = true;
                                    } else if weights.primary != 0 {
                                        last_variable = false;
                                    }
                                } else {
                                    let weight_values =
                                        vec![weights.primary, weights.secondary, weights.tertiary];
                                    cea.push(weight_values);
                                }
                            }

                            // Remove the pulled char(s) (in this order!)
                            char_vals.remove(max_right);
                            if try_two {
                                char_vals.remove(max_right - 1);
                            }

                            // Increment and continue outer loop
                            left += right - left;
                            continue 'outer;
                        }

                        // If we tried for two, don't decrement max_right yet
                        // Inner loop will run again
                        if try_two {
                            try_two = false;
                        } else {
                            // Otherwise decrement max_right; inner loop may or may not run again
                            max_right -= 1;
                        }
                    }

                    // At this point, we're not looking for a discontiguous match. We just need to
                    // push the weights we found above

                    for weights in value {
                        if shifting {
                            let weight_values = get_weights_shifting(weights, last_variable);
                            cea.push(weight_values);
                            if weights.variable {
                                last_variable = true;
                            } else if weights.primary != 0 {
                                last_variable = false;
                            }
                        } else {
                            let weight_values =
                                vec![weights.primary, weights.secondary, weights.tertiary];
                            cea.push(weight_values);
                        }
                    }

                    // Increment and continue outer loop
                    left += right - left;
                    continue 'outer;
                }

                // We failed to find the one code point
                // This means we need to skip down to deal with implicit weights
                // If we decrement right and continue middle loop, that should happen
                right -= 1;
                continue 'middle;
            }

            // If we got here, we're trying to find a slice
            let subset = &char_vals[left..right];

            if let Some(value) = multis.get(subset) {
                // If we found it, we need to check for discontiguous matches
                // Determine how much further right to look
                let mut max_right = if (right + 2) < char_vals.len() {
                    right + 2
                } else if (right + 1) < char_vals.len() {
                    right + 1
                } else {
                    // This should skip the loop below. There will be no discontiguous match
                    right
                };

                let mut try_two = max_right - right == 2 && cldr;

                'inner: while max_right > right {
                    // Need to make sure the sequence of CCCs is kosher
                    let interest_cohort = &char_vals[right..=max_right];
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
                        [subset, &char_vals[max_right - 1..=max_right]].concat()
                    } else {
                        [subset, [char_vals[max_right]].as_slice()].concat()
                    };

                    // If the new subset is found in the table...
                    if let Some(new_value) = multis.get(&new_subset) {
                        // Then add these weights instead
                        for weights in new_value {
                            if shifting {
                                let weight_values = get_weights_shifting(weights, last_variable);
                                cea.push(weight_values);
                                if weights.variable {
                                    last_variable = true;
                                } else if weights.primary != 0 {
                                    last_variable = false;
                                }
                            } else {
                                let weight_values =
                                    vec![weights.primary, weights.secondary, weights.tertiary];
                                cea.push(weight_values);
                            }
                        }

                        // Remove the pulled char(s) (in this order!)
                        char_vals.remove(max_right);
                        if try_two {
                            char_vals.remove(max_right - 1);
                        }

                        // Increment and continue outer loop
                        left += right - left;
                        continue 'outer;
                    }

                    // If we tried for two, don't decrement max_right yet
                    // Inner loop will run again
                    if try_two {
                        try_two = false;
                    } else {
                        // Otherwise decrement max_right; inner loop may or may not run again
                        max_right -= 1;
                    }
                }

                // At this point, we're not looking for a discontiguous match. We just need to push
                // the weights from the original subset we found

                for weights in value {
                    if shifting {
                        let weight_values = get_weights_shifting(weights, last_variable);
                        cea.push(weight_values);
                        if weights.variable {
                            last_variable = true;
                        } else if weights.primary != 0 {
                            last_variable = false;
                        }
                    } else {
                        let weight_values =
                            vec![weights.primary, weights.secondary, weights.tertiary];
                        cea.push(weight_values);
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

        let first_weights = get_implicit_a(left_val, shifting);
        cea.push(first_weights);

        let second_weights = get_implicit_b(left_val, shifting);
        cea.push(second_weights);

        // Finally, increment and let outer loop continue
        left += 1;
    }

    cea
}

fn get_weights_shifting(weights: &Weights, last_variable: bool) -> Vec<u16> {
    if weights.primary == 0 && weights.secondary == 0 && weights.tertiary == 0 {
        vec![0, 0, 0, 0]
    } else if weights.variable {
        vec![0, 0, 0, weights.primary]
    } else if last_variable && weights.primary == 0 && weights.tertiary != 0 {
        vec![0, 0, 0, 0]
    } else {
        vec![weights.primary, weights.secondary, weights.tertiary, 65_535]
    }
}

fn get_implicit_a(left_val: u32, shifting: bool) -> Vec<u16> {
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

    if INCLUDED_UNASSIGNED.contains(&left_val) {
        aaaa = 64_448 + (left_val >> 15);
    }

    #[allow(clippy::cast_possible_truncation)]
    let first_weights = if shifting {
        // Add an arbitrary fourth weight if shifting
        vec![aaaa as u16, 32, 2, 65_535]
    } else {
        vec![aaaa as u16, 32, 2]
    };

    first_weights
}

fn get_implicit_b(left_val: u32, shifting: bool) -> Vec<u16> {
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

    if INCLUDED_UNASSIGNED.contains(&left_val) {
        bbbb = left_val & 32_767;
    }

    // BBBB always gets bitwise ORed with this value
    bbbb |= 32_768;

    #[allow(clippy::cast_possible_truncation)]
    let second_weights = if shifting {
        // Add an arbitrary fourth weight if shifting
        vec![bbbb as u16, 0, 0, 65_535]
    } else {
        vec![bbbb as u16, 0, 0]
    };

    second_weights
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

        if k.len() < 2 {
            continue;
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

pub fn parse_fcd() -> HashMap<u32, u16> {
    let mut map = HashMap::new();

    let data = std::fs::read_to_string("test-data/UnicodeData.txt").unwrap();

    for line in data.lines() {
        if line.is_empty() {
            continue;
        }

        let splits: Vec<&str> = line.split(';').collect();

        let code_point = u32::from_str_radix(splits[0], 16).unwrap();

        let cc: u8 = splits[3].parse().unwrap();

        let decomp = splits[5];

        let mut decomp_vals: Vec<u32> = Vec::new();
        let re = regex!(r"[\dA-F]{4,5}");

        for cap in re.captures_iter(decomp) {
            decomp_vals.push(u32::from_str_radix(&cap[0], 16).unwrap());
        }

        if decomp_vals.len() == 1 {
            let full = get_canonical_decomp(splits[0]);

            decomp_vals = Vec::new();

            for cap in re.captures_iter(&full) {
                decomp_vals.push(u32::from_str_radix(&cap[0], 16).unwrap());
            }
        }

        if !decomp_vals.is_empty() {
            let first_char = char::from_u32(decomp_vals[0]).unwrap();
            let first_cc = get_ccc(first_char) as u8;

            let last_char = char::from_u32(decomp_vals[decomp_vals.len() - 1]).unwrap();
            let last_cc = get_ccc(last_char) as u8;

            let packed = ((first_cc as u16) << 8) | last_cc as u16;
            map.insert(code_point, packed);
        } else {
            let packed = ((cc as u16) << 8) | cc as u16;
            map.insert(code_point, packed);
        }
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("byte_dump", bytes).unwrap();

    map
}

fn get_canonical_decomp(code_point: &str) -> String {
    let data = std::fs::read_to_string("test-data/UnicodeData.txt").unwrap();

    for line in data.lines() {
        if line.starts_with(code_point) {
            let decomp = line.split(';').nth(5).unwrap();

            let re = regex!(r"[\dA-F]{4,5}");

            let mut decomp_vals: Vec<u32> = Vec::new();

            for cap in re.captures_iter(decomp) {
                decomp_vals.push(u32::from_str_radix(&cap[0], 16).unwrap());
            }

            if decomp_vals.len() > 1 {
                return decomp.into();
            }

            if decomp_vals.len() == 1 {
                let num_as_str = format!("{:04X}", decomp_vals[0]);
                return get_canonical_decomp(&num_as_str);
            }

            break;
        }
    }

    code_point.into()
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

        scrambled.sort_by(|a, b| collate(a, b, options));

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

        scrambled.sort_by(|a, b| collate(a, b, options));

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
