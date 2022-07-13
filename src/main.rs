#![warn(clippy::pedantic)]

use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use std::collections::HashSet;
use std::{cmp::Ordering, collections::HashMap};
use unicode_canonical_combining_class::get_canonical_combining_class as get_ccc;
use unicol_sandbox::{collate_no_tiebreak, CollationOptions, KeysSource};

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = 588;

static DECOMP: Lazy<HashMap<u32, Vec<u32>>> = Lazy::new(|| {
    let data = include_bytes!("bincode/decomp");
    let decoded: HashMap<u32, Vec<u32>> = bincode::deserialize(data).unwrap();
    decoded
});

static JAMO: Lazy<HashSet<u32>> = Lazy::new(|| {
    let data = include_bytes!("bincode/jamo");
    let decoded: HashSet<u32> = bincode::deserialize(data).unwrap();
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

        // Ignore these ranges
        if (0x3400..=0x4DBF).contains(&code_point) // CJK ext A
            || (0x4E00..=0x9FFF).contains(&code_point) // CJK
            || (0xAC00..=0xD7A3).contains(&code_point)  // Hangul
            || (0xD800..=0xDFFF).contains(&code_point) // Surrogates
            || (0xE000..=0xF8FF).contains(&code_point)  // Private use
            || (0x17000..=0x187F7).contains(&code_point) // Tangut
            || (0x18D00..=0x18D08).contains(&code_point) // Tangut suppl
            || (0x20000..=0x2A6DF).contains(&code_point) // CJK ext B
            || (0x2A700..=0x2B738).contains(&code_point) // CJK ext C
            || (0x2B740..=0x2B81D).contains(&code_point) // CJK ext D
            || (0x2B820..=0x2CEA1).contains(&code_point) // CJK ext E
            || (0x2CEB0..=0x2EBE0).contains(&code_point) // CJK ext F
            || (0x30000..=0x3134A).contains(&code_point) // CJK ext G
            || (0xF0000..=0xFFFFD).contains(&code_point) // Plane 15 private use
            // Plane 16 private use
            || (1_048_576..=1_114_109).contains(&code_point)
        {
            continue;
        }

        let decomp_col = splits[5];

        let re = regex!(r"[\dA-F]{4,5}");

        let mut decomp: Vec<u32> = Vec::new();

        for cap in re.captures_iter(decomp_col) {
            decomp.push(u32::from_str_radix(&cap[0], 16).unwrap());
        }

        let final_decomp = if decomp_col.contains('<') {
            // Non-canonical decomposition; continue
            continue;
        } else if decomp.len() > 1 {
            // Multi-code-point canonical decomposition; recurse badly
            decomp
                .into_iter()
                .flat_map(|x| {
                    let as_str = format!("{:04X}", x);
                    get_canonical_decomp(&as_str)
                })
                .collect::<Vec<u32>>()
        } else if decomp.len() == 1 {
            // Single-code-point canonical decomposition; recurse simply
            get_canonical_decomp(splits[0])
        } else {
            // No decomposition; continue
            continue;
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

            // Further decomposition is non-canonical; return the code point itself
            if decomp_col.contains('<') {
                return vec![u32::from_str_radix(code_point, 16).unwrap()];
            }

            let re = regex!(r"[\dA-F]{4,5}");

            let mut decomp: Vec<u32> = Vec::new();

            for cap in re.captures_iter(decomp_col) {
                decomp.push(u32::from_str_radix(&cap[0], 16).unwrap());
            }

            // Further multiple-code-point decomposition; recurse badly
            if decomp.len() > 1 {
                return decomp
                    .into_iter()
                    .flat_map(|x| {
                        let as_str = format!("{:04X}", x);
                        get_canonical_decomp(&as_str)
                    })
                    .collect::<Vec<u32>>();
            }

            // Further single-code-point decomposition; recurse simply
            if decomp.len() == 1 {
                let as_str = format!("{:04X}", decomp[0]);
                return get_canonical_decomp(&as_str);
            }

            // No further decomposition; return the code point itself
            return vec![u32::from_str_radix(code_point, 16).unwrap()];
        }
    }

    // This means we followed a canonical decomposition to a single code point that was then not
    // found in the first column of the table. Return it, I guess?
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

        // Ignore these ranges
        if (0x3400..=0x4DBF).contains(&code_point) // CJK ext A
            || (0x4E00..=0x9FFF).contains(&code_point) // CJK
            || (0xAC00..=0xD7A3).contains(&code_point)  // Hangul
            || (0xD800..=0xDFFF).contains(&code_point) // Surrogates
            || (0xE000..=0xF8FF).contains(&code_point)  // Private use
            || (0x17000..=0x187F7).contains(&code_point) // Tangut
            || (0x18D00..=0x18D08).contains(&code_point) // Tangut suppl
            || (0x20000..=0x2A6DF).contains(&code_point) // CJK ext B
            || (0x2A700..=0x2B738).contains(&code_point) // CJK ext C
            || (0x2B740..=0x2B81D).contains(&code_point) // CJK ext D
            || (0x2B820..=0x2CEA1).contains(&code_point) // CJK ext E
            || (0x2CEB0..=0x2EBE0).contains(&code_point) // CJK ext F
            || (0x30000..=0x3134A).contains(&code_point) // CJK ext G
            || (0xF0000..=0xFFFFD).contains(&code_point) // Plane 15 private use
            // Plane 16 private use
            || (1_048_576..=1_114_109).contains(&code_point)
        {
            continue;
        }

        let can_decomp = match DECOMP.get(&code_point) {
            Some(cd) => cd,
            None => continue,
        };

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

#[allow(unused)]
fn decompose(input: &mut Vec<u32>) {
    let mut i: usize = 0;

    while i < input.len() {
        if input[i] >= 0xAC00 && input[i] <= 0xD7A3 {
            let rep = decompose_jamo(input[i]);
            let n = rep.len();
            input.splice(i..=i, rep);
            i += n;
            continue;
        }

        if let Some(rep) = DECOMP.get(&input[i]) {
            input.splice(i..=i, rep.clone());
            i += rep.len();
            continue;
        }

        i += 1;
    }
}

fn decompose_jamo(s: u32) -> Vec<u32> {
    let s_index = s - S_BASE;

    let lv = JAMO.get(&s).is_some();

    if lv {
        let l_index = s_index / N_COUNT;
        let v_index = (s_index % N_COUNT) / T_COUNT;

        let l_part = L_BASE + l_index;
        let v_part = V_BASE + v_index;

        vec![l_part, v_part]
    } else {
        let l_index = s_index / N_COUNT;
        let v_index = (s_index % N_COUNT) / T_COUNT;
        let t_index = s_index % T_COUNT;

        let l_part = L_BASE + l_index;
        let v_part = V_BASE + v_index;
        let t_part = T_BASE + t_index;

        vec![l_part, v_part, t_part]
    }
}

#[allow(unused)]
fn reorder(input: &mut Vec<u32>) {
    let mut n = input.len();

    while n > 1 {
        let mut new_n = 0;

        let mut i = 1;

        while i < n {
            let ccc_b = get_ccc(char::from_u32(input[i]).unwrap()) as u8;

            if ccc_b == 0 {
                i += 2;
                continue;
            }

            let ccc_a = get_ccc(char::from_u32(input[i - 1]).unwrap()) as u8;

            if ccc_a == 0 || ccc_a <= ccc_b {
                i += 1;
                continue;
            }

            input.swap(i - 1, i);

            new_n = i;
            i += 1;
        }

        n = new_n;
    }
}
