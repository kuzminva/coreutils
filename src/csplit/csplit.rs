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

    if matches.opt_present("h") {
        let msg = format!("{0} {1}

Usage:
  {0} [OPTION]... [INPUT [PREFIX]]

Output fixed-size pieces of INPUT to PREFIXaa, PREFIX ab, ...; default
size is 1000, and default PREFIX is 'x'. With no INPUT, or when INPUT is
-, read standard input.", NAME, VERSION);

        println!("{}\nSIZE may have a multiplier suffix: b for 512, k for 1K, m for 1 Meg.", opts.usage(&msg));
        return 0;
    }

    if matches.opt_present("V") {
        println!("{} {}", NAME, VERSION);
        return 0;
    }

    let mut settings = Settings {
        prefix: "".to_owned(),
        numeric_suffix: false,
        suffix_length: 0,
        input: "".to_owned(),
        strategy: "".to_owned(),
        strategy_param: "".to_owned(),
        verbose: false,
    };

    settings.numeric_suffix = matches.opt_present("d");

    settings.suffix_length = match matches.opt_str("a") {
        Some(n) => match n.parse() {
            Ok(m) => m,
            Err(e) => crash!(1, "cannot parse num: {}", e)
        },
        None => 2
    };

    settings.verbose = matches.opt_present("verbose");

    settings.strategy = "l".to_owned();
    settings.strategy_param = "1000".to_owned();
    let strategies = vec!["b", "C", "l"];
    for e in &strategies {
        match matches.opt_str(*e) {
            Some(a) => {
                if settings.strategy == "l" {
                    settings.strategy = (*e).to_owned();
                    settings.strategy_param = a;
                } else {
                    crash!(1, "{}: cannot csplit in more than one way", NAME)
                }
            },
            None => {}
        }
    }

    let mut v = matches.free.iter();
    let (input, prefix) = match (v.next(), v.next()) {
        (Some(a), None) => (a.to_owned(), "x".to_owned()),
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        (None, _) => ("-".to_owned(), "x".to_owned()),
    };
    settings.input = input;
    settings.prefix = prefix;

    csplit(&settings)
}

struct Settings {
    prefix: String,
    numeric_suffix: bool,
    suffix_length: usize,
    input: String,
    strategy: String,
    strategy_param: String,
    verbose: bool,
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
    saved_lines_to_write: usize,
    lines_to_write: usize,
}

impl LineCsplitter {
    fn new(settings: &Settings) -> Box<Csplitter> {
        let n = match settings.strategy_param.parse() {
            Ok(a) => a,
            Err(e) => crash!(1, "invalid number of lines: {}", e)
        };
        Box::new(LineCsplitter {
            saved_lines_to_write: n,
            lines_to_write: n,
        }) as Box<Csplitter>
    }

}

impl Csplitter for LineCsplitter {
    fn consume(&mut self, control: &mut CsplitControl) -> String {
        self.lines_to_write -= 1;
        if self.lines_to_write == 0 {
            self.lines_to_write = self.saved_lines_to_write;
            control.request_new_file = true;
        }
        control.current_line.clone()
    }
}

struct ByteCsplitter {
    saved_bytes_to_write: usize,
    bytes_to_write: usize,
    break_on_line_end: bool,
    require_whole_line: bool,
}

impl ByteCsplitter {
    fn new(settings: &Settings) -> Box<Csplitter> {
        let mut strategy_param : Vec<char> = settings.strategy_param.chars().collect();
        let suffix = strategy_param.pop().unwrap();
        let multiplier = match suffix {
            '0'...'9' => 1usize,
            'b' => 512usize,
            'k' => 1024usize,
            'm' => 1024usize * 1024usize,
            _ => crash!(1, "invalid number of bytes")
        };
        let n = if suffix.is_alphabetic() {
            match strategy_param.iter().cloned().collect::<String>().parse::<usize>() {
                Ok(a) => a,
                Err(e) => crash!(1, "invalid number of bytes: {}", e)
            }
        } else {
            match settings.strategy_param.parse::<usize>() {
                Ok(a) => a,
                Err(e) => crash!(1, "invalid number of bytes: {}", e)
            }
        };
        Box::new(ByteCsplitter {
            saved_bytes_to_write: n * multiplier,
            bytes_to_write: n * multiplier,
            break_on_line_end: settings.strategy == "b",
            require_whole_line: false,
        }) as Box<Csplitter>
    }
}

impl Csplitter for ByteCsplitter {
    fn consume(&mut self, control: &mut CsplitControl) -> String {
        let line = control.current_line.clone();
        let n = std::cmp::min(line.chars().count(), self.bytes_to_write);
        if self.require_whole_line && n < line.chars().count() {
            self.bytes_to_write = self.saved_bytes_to_write;
            control.request_new_file = true;
            self.require_whole_line = false;
            return line[0..0].to_owned();
        }
        self.bytes_to_write -= n;
        if n == 0 {
            self.bytes_to_write = self.saved_bytes_to_write;
            control.request_new_file = true;
        }
        if self.break_on_line_end && n == line.chars().count() {
            self.require_whole_line = self.break_on_line_end;
        }
        line[..n].to_owned()
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
            "b" | "C" => ByteCsplitter::new(settings),
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
            filename.push_str(if settings.numeric_suffix {
                num_prefix(fileno, settings.suffix_length)
            } else {
                str_prefix(fileno, settings.suffix_length)
            }.as_ref());

            if fileno != 0 {
                crash_if_err!(1, writer.flush());
            }
            fileno += 1;
            writer = BufWriter::new(Box::new(OpenOptions::new().write(true).create(true).open(Path::new(&filename)).unwrap()) as Box<Write>);
            control.request_new_file = false;
            if settings.verbose {
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
