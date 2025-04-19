use std::path::Path;

use globby::{Pattern, PatternOpts, Walker};

fn main() {
    let pattern = Pattern::new_with_opts(
        "**/*.*",
        PatternOpts {
            case_insensitive: false,
        },
    )
    .unwrap();

    let walker = Walker::new(pattern, Path::new("/"));

    for path in walker {
        match path {
            Ok(path) => println!("OK: {}", path.display()),
            Err(err) => println!("ERR: {err}"),
        }
    }
}
