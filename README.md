# unicol-sandbox

I've been trying to get a Rust implementation of the [Unicode Collation Algorithm](https://unicode.org/reports/tr10/) working. At last, my spaghetti code fully passes both of the [conformance tests](https://www.unicode.org/Public/UCA/latest/CollationTest.html)—the "non-ignorable" and "shifted" variants. More work will follow to clean up the code, make things faster (hopefully), turn it into a library, etc.

For now, you can run this yourself: `cargo run --release`. The program iterates over a test file, making sure that each line orders greater than or equal to the one preceding it.

Again, I'm sure this is a slow implementation. I wanted to start by achieving conformance.
