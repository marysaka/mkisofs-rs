extern crate byteorder;
extern crate chrono;

extern crate structopt;

use structopt::StructOpt;

mod iso;

use iso::option::Opt;

fn main() {
    let mut opt = Opt::from_args();
    iso::create_iso(&mut opt).unwrap();
}
