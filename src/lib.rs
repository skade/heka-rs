#![allow(unstable)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]
//#![feature(plugin)]
//#![plugin(regex_macros)]

extern crate libc;
extern crate protobuf;
extern crate regex;
//extern crate regex_macros;
extern crate uuid;
#[macro_use]
extern crate lazy_static;

pub mod message;
pub mod sandbox;
pub mod splitter;
