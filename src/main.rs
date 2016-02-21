#![allow(unstable)]

extern crate protobuf; // depend on rust-protobuf runtime
extern crate getopts;
extern crate heka;

use getopts::Options;

use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

use protobuf::clear::Clear;
use protobuf::Message;
use heka::message::pb;
use heka::{message,sandbox,splitter};

fn print_usage(program: &str, opts: &Options) {
    let brief = format!("Usage: {} [options] <input_file>", program);
    println!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("m", "match", "set the message matcher filter", "TRUE");
    opts.optopt("p", "plugin", "set plugin name (will look for the toml in the same location)", "plugin.lua");
    opts.optopt("o", "output", "set output file name", "heka_cat.hpb");
    opts.optflag("h", "help", "print this help menu");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => {
            println!("{}\n", f.to_string());
            print_usage(program.as_ref(), &opts);
            return;
        }
    };
    if matches.opt_present("h") {
        print_usage(program.as_ref(), &opts);
        return;
    }
    let input = if !matches.free.is_empty() {
        matches.free[0].clone()
    } else {
        print_usage(program.as_ref(), &opts);
        return;
    };


    let m = match matches.opt_str("m") {
        Some(m) => m,
        None => "TRUE".to_string(),
    };
    let mm = match message::matcher::Matcher::new(m.as_ref()) {
        Ok(m) => m,
        Err(e) => panic!("invalid match at position({}): {}", e.pos, e.msg),
    };
    let path = Path::new(&input);
    let mut hps = splitter::HekaProtobufStream::new(File::open(&path).expect("File couldn't be opened."), 1024*64+255+3); // max message size + header + seperators
    let mut lsb = sandbox::LuaSandbox::new("../test/fxa_active_daily_users.lua".as_bytes(), "heka_rs/modules".as_bytes(), 0, 0, 0);
    let preservation = "fxa_active_daily_users.preserve";
    let r = if Path::new(preservation).exists() {
        lsb.init(preservation.as_bytes())
    } else {
        lsb.init("".as_bytes())
    };
    if r != 0 {
        panic!("sandbox_init failed {} {}", r, lsb.last_error());
    }

    let mut count = 0;
    let mut match_count = 0;
    let mut msg = Some(Box::new(pb::HekaMessage::new()));
    loop {
        match hps.read_next() {
            Ok(m) => {
                if m.is_some() {
                    let m = m.unwrap();
                    count += 1;
                    let mut reader = BufReader::new(m);
                    let mut cis = protobuf::CodedInputStream::new(&mut reader);
                    msg.as_mut().unwrap().clear();
                    msg.as_mut().unwrap().merge_from(&mut cis); // todo: warning this asserts on corrupt records
                    if msg.as_ref().unwrap().is_initialized() {
                        if mm.is_match(&**msg.as_ref().unwrap()) {
                            match_count += 1;
                            let (rc, mm) = lsb.process_message(msg.take().unwrap());
                            msg = Some(mm);
                            if rc > 0 {
                                println!("process message failed {} {}", rc, lsb.last_error());
                                return;
                            } else if rc == -1 {
                                println!("process message failed parsing line {}: {}", count, msg.as_ref().unwrap().get_payload());
                            }
                        }
                    } else {
                        println!("missing required field");
                    }
                }
            }
            Err(e) => {
                match e.kind() {
                    std::io::ErrorKind::UnexpectedEof => { break; }
                    std::io::ErrorKind::Other => { println!("non-fatal read error: {}", e); }
                    _ => {
                        println!("read error: {}", e);
                        break;
                    }
                }
            }
        }
    }
    let err = lsb.last_error();
    if !err.is_empty() {
        println!("last error {}", err);
    }
    lsb.destroy(preservation.as_bytes());
    println!("offset: {} total_count: {} match_count: {}", hps.tell().unwrap_or(0), count, match_count);
}
