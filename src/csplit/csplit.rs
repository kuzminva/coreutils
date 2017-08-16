#![crate_name = "uu_csplit"]

/*
 * This file is part of the uutils coreutils package.
 *
 * (c) Akira Hayakawa <ruby.wktk@gmail.com>
 * (c) Vladimir Kuzmin <kuzminva@gmail.com>
 * (c) Svetlana Avetisyan <svetlana.avetisyan.1992@mail.ru>
 * (c) Touseef Liaqat <touseefliaqat@gmail.com>
 *
 * For the full copyright and license information, please view the LICENSE
 * file that was distributed with this source code.
 */

extern crate getopts;

#[macro_use]
extern crate uucore;

use std::char;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, stdin, stdout, Write};
use std::path::Path;

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

    let mut settings = Settings {
        suffix_format: "%0{}d".to_owned(),
        prefix: "xx".to_owned(),
        suppress_matched: false,
        digits: 2,
        silent: false,
        elide_empty_files: false,
        input: "".to_owned(),
        strategy: "".to_owned(),
        numbers: vec![std::usize::MAX]
    };

    settings.silent = matches.opt_present("silent");
    settings.strategy = "lines".to_owned();
    let mut v = matches.free.iter();
    settings.input = match v.next() {
        Some(a) => a.to_owned(),
        None => "-".to_owned(),
    };

    for val in matches.free {
      match val.parse::<usize>() {
         Ok(n) => settings.numbers.push(n),
         Err(e) => crash!(1, "Does't support patterns yet: {}", e)
       }
    }

    csplit(&settings)
}

struct Settings {
    suffix_format: String,
    prefix: String,
    suppress_matched: bool,
    digits: usize,
    silent: bool,
    elide_empty_files: bool,
    input: String,
    strategy: String,
    numbers: Vec<usize>
}

struct CsplitControl {
    current_line: String, // Don't touch
    request_new_file: bool, // Csplitter implementation requests new file
}

trait Csplitter {
    // Consume the current_line and return the consumed string
    fn consume(&mut self, &mut CsplitControl) -> String;
}

struct LineCsplitter {
    numbers: Vec<usize>,
    index: usize,
    lines_to_write: usize,
}

impl LineCsplitter {
    fn new(settings: &Settings) -> Box<Csplitter> {

        let v = settings.numbers;
        if settings.numbers.is_empty() {
            v = vec![std::usize::MAX];
        }

        Box::new(LineCsplitter {
            numbers: v,
            index: 0,
            lines_to_write: v[0]
        }) as Box<Csplitter>
    }

}

impl Csplitter for LineCsplitter {
    fn consume(&mut self, control: &mut CsplitControl) -> String {
        self.lines_to_write -= 1;
        if self.lines_to_write == 0 {
            self.index += 1;
            if self.index < self.numbers.len() {
               self.lines_to_write = self.numbers[self.index];
               control.request_new_file = true;
            } else {
               self.lines_to_write = std::usize::MAX;
            } 
        }
        control.current_line.clone()
    }
}


// (1, 3) -> "aab"
fn str_prefix(i: usize, width: usize) -> String {
    let mut c = "".to_owned();
    let mut n = i;
    let mut w = width;
    while w > 0 {
        w -= 1;
        let div = 26usize.pow(w as u32);
        let r = n / div;
        n -= r * div;
        c.push(char::from_u32((r as u32) + 97).unwrap());
    }
    c
}

// (1, 3) -> "001"
fn num_prefix(i: usize, width: usize) -> String {
    let mut c = "".to_owned();
    let mut n = i;
    let mut w = width;
    while w > 0 {
        w -= 1;
        let div = 10usize.pow(w as u32);
        let r = n / div;
        n -= r * div;
        c.push(char::from_digit(r as u32, 10).unwrap());
    }
    c
}

fn csplit(settings: &Settings) -> i32 {
    let mut reader = BufReader::new(
        if settings.input == "-" {
            Box::new(stdin()) as Box<Read>
        } else {
            let r = match File::open(Path::new(&settings.input)) {
                Ok(a) => a,
                Err(_) => crash!(1, "cannot open '{}' for reading: No such file or directory", settings.input)
            };
            Box::new(r) as Box<Read>
        }
    );

    let mut csplitter: Box<Csplitter> =
        match settings.strategy.as_ref() {
            "l" => LineCsplitter::new(settings),
            a => crash!(1, "strategy {} not supported", a)
        };

    let mut control = CsplitControl {
        current_line: "".to_owned(), // Request new line
        request_new_file: true, // Request new file
    };

    let mut writer = BufWriter::new(Box::new(stdout()) as Box<Write>);
    let mut fileno = 0;
    loop {
        if control.current_line.chars().count() == 0 {
            match reader.read_line(&mut control.current_line) {
                Ok(0) | Err(_) => break,
                _ => {}
            }
        }

        if control.request_new_file {
            let mut filename = settings.prefix.clone();
            filename.push_str(
                num_prefix(fileno, settings.digits).as_ref());

            if fileno != 0 {
                crash_if_err!(1, writer.flush());
            }
            fileno += 1;
            writer = BufWriter::new(Box::new(OpenOptions::new().write(true).create(true).open(Path::new(&filename)).unwrap()) as Box<Write>);
            control.request_new_file = false;
            if settings.silent {
                println!("creating file '{}'", filename);
            }
        }

        let consumed = csplitter.consume(&mut control);
        crash_if_err!(1, writer.write_all(consumed.as_bytes()));

        let advance = consumed.chars().count();
        let clone = control.current_line.clone();
        let sl = clone;
        control.current_line = sl[advance..sl.chars().count()].to_owned();
    }
    0
}
