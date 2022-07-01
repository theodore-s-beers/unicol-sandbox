# unicol-sandbox

I'm trying to get a Rust implementation of the [Unicode Collation Algorithm](https://unicode.org/reports/tr10/) working. At this point, my spaghetti code is extremely close to passing one of the conformance tests (the "non-ignorable" variant). I just had to comment out eight lines, out of 200,000+, in the test file. (There's an issue with a few Tibetan characters that I haven't been able to figure out. I don't think it's my fault; there are caveats about Tibetan in the UCA docs.)

You can run this yourself: `cargo run --release`. The program iterates over the test file, making sure that each line orders greater than or equal to the one preceding it.

More work will follow. I wanted to start by achieving basic conformance.
