use criterion::{criterion_group, criterion_main, Criterion};
use std::cmp::Ordering;
use unicol_sandbox::{compare_sort_keys, str_to_sort_key, CollationOptions, KeysSource};

fn conformance(path: &str, options: CollationOptions) {
    let test_data = std::fs::read_to_string(path).unwrap();

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
}

fn ducet_ni(c: &mut Criterion) {
    c.bench_function("DUCET, non-ignorable", |b| {
        b.iter(|| {
            conformance(
                "test-data/CollationTest_NON_IGNORABLE_SHORT.txt",
                CollationOptions {
                    keys_source: KeysSource::Ducet,
                    shifting: false,
                },
            )
        })
    });
}

fn ducet_shifted(c: &mut Criterion) {
    c.bench_function("DUCET, shifted", |b| {
        b.iter(|| {
            conformance(
                "test-data/CollationTest_SHIFTED_SHORT.txt",
                CollationOptions {
                    keys_source: KeysSource::Ducet,
                    shifting: true,
                },
            )
        })
    });
}

fn cldr_ni(c: &mut Criterion) {
    c.bench_function("CLDR, non-ignorable", |b| {
        b.iter(|| {
            conformance(
                "test-data/CollationTest_CLDR_NON_IGNORABLE_SHORT.txt",
                CollationOptions {
                    keys_source: KeysSource::Cldr,
                    shifting: false,
                },
            )
        })
    });
}

fn cldr_shifted(c: &mut Criterion) {
    c.bench_function("CLDR, shifted", |b| {
        b.iter(|| {
            conformance(
                "test-data/CollationTest_CLDR_SHIFTED_SHORT.txt",
                CollationOptions {
                    keys_source: KeysSource::Cldr,
                    shifting: true,
                },
            )
        })
    });
}

criterion_group!(benches, ducet_ni, ducet_shifted, cldr_ni, cldr_shifted);
criterion_main!(benches);
