
extern crate structopt;
use structopt::StructOpt;

#[derive(Clone, Debug, StructOpt)]
pub struct Options {



}



fn main() {
    let o = Options::from_args();

    println!("Options: {:?}", o);

}


