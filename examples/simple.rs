use globby::glob;

fn main() {
    let pattern = glob("**/*.*").unwrap();

    for path in pattern {
        println!("{}", path.unwrap().display());
    }
}
