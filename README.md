# Globby

Globby is a small ~1k LoC library designed for searching all items in a given directory that match a *glob pattern*.

## Examples

```rust
use globby::glob;

let pattern = glob("**/*.*").unwrap();

for path in pattern {
  println!("{}", path.unwrap().display());
}
```

This library should work on any platform.

## Comparing to [`glob`](https://docs.rs/glob)

The well-known glob library is more polished and has a lot more options, but also opinionated defaults that differ from this library, such as:

* The base directory is not yielded in the results
* Symbolic links are always followed
* Directories are always yielded before their descendents
* Alternate groups (matching either one sub-pattern or another) is supported
* `**` matches anything, including files an hidden directories

## Syntax

The syntax for patterns is as followed:

* Normal characters behave as expected
* `?` matches any character
* `*` matches any suite of characters, or no character at all
* `[abc]` matches any of `a`, `b` or `c`
* `[!abc]` matches any character except `a`, `b` and `c`
* `[\[]` matches `[`. The list of escapable characters is `[`, `]`, `{`, `}`, `*`, `?`, `\`, `/`, `|` and `:`
    - `[abc\[]` matches any of `a`, `b`, `c` or `[`
* `[[:alpha:]]` will match any alphabetic character. The list of character classes are:
    - `:alpha:` for any alphabetic character
    - `:digit:` for any digit
    - `:alphanumeric:` for any alphabetic character or digit
    - `:uppercase:` for any uppercase character
    - `:lowercase:` for any lowercase character
    - `:whitespace:` for any whitespace character
* `[![:alpha:]]` will match any non-alphabetic character
* `{a|bc}` will match any of `a` or `bc`
    - This can be combined with other matchers, e.g. `{[[:alpha:]][![:digit]]|[[:digit:]]*}` will match any alphabetic character followed by a non-digit character, OR a digit followed by anything

Matches are performed against path components, e.g. in `/path/to/item` components are `path`, `to` and `item`.
Matchers **cannot** match path separators.

* Path separators can be written as `/` or `\` depending on the platform (Windows, Linux, macOS, ...)
* In addition, `**` will match any possible combination of directories. For instance, `/**/*.txt` will match any of `/file.txt`, `/dir/file.txt`, `/dir/dir2/file.txt`, and so on.
* Absolute patterns can only be matched against absolute paths. e.g. `/dir` will not match `dir`. Note that using a [`crate::Walker`] will not cause this problem as a base directory is used.

## License

This project is provided under the terms of the [Apache 2.0 license](./LICENSE.md).
