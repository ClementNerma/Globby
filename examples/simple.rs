use globby::glob_current_dir;

fn main() {
    let pattern = glob_current_dir("**/*.*").unwrap();

    for path in pattern {
        println!("{}", path.unwrap().display());
    }
}
