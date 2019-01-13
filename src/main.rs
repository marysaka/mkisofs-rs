extern crate byteorder;
extern crate chrono;

use std::env;

mod iso;

fn main() {
    let mut args = env::args();
    let executable_path = args.next().unwrap();

    match args.len() {
        2 => {
            let output_path = args.next().unwrap();
            let input_directory = args.next().unwrap();
            println!("Output file {}", output_path);
            println!("Input directory {}", input_directory);
            iso::create_iso(output_path, input_directory).unwrap();
        }
        _ => println!("Usage: {} out.iso input_directory", executable_path),
    }
}
