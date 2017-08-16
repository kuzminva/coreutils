#![crate_name = "uu_csplit"]

/*
 * This file is a part of the uutils coreutils package.
 *
 * (c) Vladimir Kuzmin <kuzminva@gmail.com>
 *
 * For the full copyright and licenst information, please view the LICENSE
 * file that was distributed with this source code.
 *
 */

extern crate getopts;

#[macro_use]
extern crate uucore;

use std::io::{Write};

static NAME: &'static str = "csplit";
static VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub fn uumain(args: Vec<String>) -> i32 {
   let mut opts = getopts::Options::new();

   opts.optopt("b", "suffix-format", "use sprintf FORMAT instead of %02d", "FORMAT");
   opts.optopt("f", "prefix=PREFIX", "use PREFIX instead of \'xx\'", "PREFIX");
   opts.optflag("", "supress-matched", "supress the lines matching PATTERN");
   opts.optopt("n", "digits", "use specified number of digits instead of 2", "DIGITS");
   opts.optflag("s", "quite,silent", "do not print counts of output file sizes");
   opts.optflag("z", "elide-empty-files", "remove empty output files");
   opts.optflag("", "help", "display this help and exit");
   opts.optflag("", "version", "output version information and exit");
   
   let matches = match opts.parse(&args[1..]) {
      Ok(m) => m,
      Err(f) => crash!(1, "{}", f)
   };
   
   if matches.opt_present("help") {
      let msg = format!("{0} {1}

Usage:
{0} [OPTION]... FILE PATTERN...
Output pieces of FILE separated by PATTERN(s) to files \'xx00\', \'xx01\', ...,
and output byte counts of each piece to standard output. 
When FILE is -, read standard input.", NAME, VERSION);
      println!("{}\n Each PATTERN maybe: INTEGER, REGEX", opts.usage(&msg)); 
      return 0;
   }

   if matches.opt_present("version") {
      println!("{} {}", NAME, VERSION);
      return 0;
   }

   return 0;
}


 
