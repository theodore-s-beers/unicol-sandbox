#![warn(clippy::pedantic)]

use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use std::{cmp::Ordering, collections::HashMap};
use unicode_canonical_combining_class::get_canonical_combining_class as get_ccc;
use unicol_sandbox::{collate_no_tiebreak, CollationOptions, KeysSource};

static DECOMP: Lazy<HashMap<u32, Vec<u32>>> = Lazy::new(|| {
    let data = include_bytes!("bincode/decomp");
    let decoded: HashMap<u32, Vec<u32>> = bincode::deserialize(data).unwrap();
    decoded
});

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceCell<Regex> = OnceCell::new();
        RE.get_or_init(|| Regex::new($re).unwrap())
    }};
}

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

#[allow(unused)]
fn map_decomps() {
    let data = std::fs::read_to_string("test-data/UnicodeData.txt").unwrap();

    let mut map: HashMap<u32, Vec<u32>> = HashMap::new();

    for line in data.lines() {
        if line.is_empty() {
            continue;
        }

        let splits: Vec<&str> = line.split(';').collect();

        let code_point = u32::from_str_radix(splits[0], 16).unwrap();

        let decomp_col = splits[5];

        let re = regex!(r"[\dA-F]{4,5}");

        let mut decomp: Vec<u32> = Vec::new();

        for cap in re.captures_iter(decomp_col) {
            decomp.push(u32::from_str_radix(&cap[0], 16).unwrap());
        }

        let final_decomp = if code_point > 55_295 && code_point < 57_344 {
            // Surrogate code point; return FFFD
            vec![65_533]
        } else if decomp_col.contains('<') {
            // Non-canonical decomposition; return the code point itself
            vec![code_point]
        } else if decomp.len() > 1 {
            // Multi-code-point canonical decomposition; return it
            decomp
        } else if decomp.len() == 1 {
            // Single-code-point canonical decomposition; dig deeper
            get_canonical_decomp(splits[0])
        } else {
            // No decomposition; return the code point itself
            vec![code_point]
        };

        map.insert(code_point, final_decomp);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("byte_dump", bytes).unwrap();
}

fn get_canonical_decomp(code_point: &str) -> Vec<u32> {
    let data = std::fs::read_to_string("test-data/UnicodeData.txt").unwrap();

    for line in data.lines() {
        if line.starts_with(code_point) {
            let decomp_col = line.split(';').nth(5).unwrap();

            // Non-canonical decomposition; return the code point itself
            if decomp_col.contains('<') {
                return vec![u32::from_str_radix(code_point, 16).unwrap()];
            }

            let re = regex!(r"[\dA-F]{4,5}");

            let mut decomp: Vec<u32> = Vec::new();

            for cap in re.captures_iter(decomp_col) {
                decomp.push(u32::from_str_radix(&cap[0], 16).unwrap());
            }

            // Multiple-code-point decomposition; return it
            if decomp.len() > 1 {
                return decomp;
            }

            // Recurse
            if decomp.len() == 1 {
                let as_str = format!("{:04X}", decomp[0]);
                return get_canonical_decomp(&as_str);
            }

            // No further decomposition; return the code point itself
            return vec![u32::from_str_radix(code_point, 16).unwrap()];
        }
    }

    // This means we followed a canonical decomposition to a single code point that was then not
    // found in the table. Return it, I guess?
    vec![u32::from_str_radix(code_point, 16).unwrap()]
}

#[allow(unused)]
fn map_fcd() {
    let data = std::fs::read_to_string("test-data/UnicodeData.txt").unwrap();

    let mut map: HashMap<u32, u16> = HashMap::new();

    for line in data.lines() {
        if line.is_empty() {
            continue;
        }

        let left_of_semicolon = line.split(';').next().unwrap();

        let code_point = u32::from_str_radix(left_of_semicolon, 16).unwrap();

        let can_decomp = DECOMP.get(&code_point).unwrap();

        let decomp_first_ch = char::from_u32(can_decomp[0]).unwrap();
        let first_cc = get_ccc(decomp_first_ch) as u8;

        let decomp_last_ch = char::from_u32(can_decomp[can_decomp.len() - 1]).unwrap();
        let last_cc = get_ccc(decomp_last_ch) as u8;

        let packed = (u16::from(first_cc) << 8) | u16::from(last_cc);
        map.insert(code_point, packed);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("byte_dump", bytes).unwrap();
}
