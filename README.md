# unicol-sandbox

I'm trying to get a Rust implementation of the [Unicode Collation Algorithm](https://unicode.org/reports/tr10/) working. At this point, my spaghetti code fully passes one of the [conformance tests](https://www.unicode.org/Public/UCA/latest/CollationTest.html) ("non-ignorable"), and is very close to passing the other ("shifted").

You can run this yourself: `cargo run --release`. The program iterates over a test file, making sure that each line orders greater than or equal to the one preceding it.

More work will follow. I'm sure this is a slow implementation. I wanted to start by achieving conformance.
