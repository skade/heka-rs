extern crate rlibc;

use std::io::{BufReader};
use std::io;
use std::io::prelude::*;
use std::vec::Vec;
use protobuf::{CodedInputStream, Message};
use protobuf::clear::Clear;
use super::message::pb;

static RECORD_SEP: u8 = 0x1e;

//pub trait Splitter {
//    // Finds the next record in the stream.
//    // Returns the number of bytes read from the stream. This will not always
//    // correlate to the record size since delimiters can be discarded and data
//    // corruption skipped which also means the record could be empty even if
//    // bytes were read. 'record' will remain valid until the next call to split.
//    // The bytes read can be non zero even when there is an IoError.
//    fn read_next<'a>(&'a mut self) -> (usize, io::Result<Option<&'a [u8]>>);
//
//    // Reads the remainder of the parse buffer.  This is the
//    // only way to fetch the last record in a stream that specifies a start of
//    // line  delimiter or contains a partial last line.  It should only be
//    // called when no additional data will be appended to the stream.
//    fn read_remaining<'a>(&'a mut self) -> &'a [u8];
//
//    // tracks the absolute offset in the stream
//    fn tell(&self) -> io::Result<u64>;
//}


pub struct HekaProtobufStream<R> {
    reader: R,
    cap: usize,
    buf: Vec<u8>,
    scan_pos: usize,
    read_pos: usize,
    offset: u64,
    header: pb::Header,
}

impl<R: Read> HekaProtobufStream<R> {
    pub fn new(reader: R, cap: usize) -> HekaProtobufStream<R> {
        let mut buf = Vec::with_capacity(cap);
        unsafe { buf.set_len(cap); }
        HekaProtobufStream {
            reader: reader,
            cap: cap,
            buf: buf,
            scan_pos: 0,
            read_pos: 0,
            offset: 0u64,
            header: pb::Header::new(),
        }
    }

    pub fn read_next<'a>(&'a mut self) -> io::Result<Option<&'a [u8]>> {
        let required = 258 + self.header.get_message_length() as usize;
        match self.read(required) {
            Ok(_) => self.find_record(),
            Err(e) => Err(e)
        }
    }

    pub fn read_remaining<'a>(&'a mut self) -> Option<&'a [u8]> {
        if self.read_pos - self.scan_pos > 0 {
            let r = Some(&self.buf[self.scan_pos..self.read_pos]);
            self.offset += (self.read_pos - self.scan_pos) as u64;
            self.scan_pos = 0;
            self.read_pos = 0;
            return r;
        }
        None
    }

    pub fn tell(&self) -> io::Result<u64> {
        Ok(self.offset)
    }

    fn decode_header(&mut self, header_end: usize) -> bool {
        if self.buf[header_end-1] != 0x1f {
            return false;
        }
        let mut reader = BufReader::new(&self.buf[self.scan_pos+2..header_end-1]);
        let mut cis = CodedInputStream::new(&mut reader);
        self.header.merge_from(&mut cis); // todo: warning this asserts on corrupt records
        if self.header.is_initialized() {
            return true;
        }
        false
    }

    fn read(&mut self, required: usize) -> io::Result<usize> {
        if required > self.cap {
            self.offset += (self.read_pos - self.scan_pos) as u64;
            self.read_pos = 0;
            self.scan_pos = 0;
            return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Record exceeds capacity"
                    ));
        }
        if self.scan_pos == self.read_pos {
            self.scan_pos = 0;
            self.read_pos = 0;
        } else if self.scan_pos + required >= self.cap {
            self.shift_buffer();
        } else {
            return Ok(0); // buffer already contains enough data
        }
        match self.reader.read(&mut self.buf[self.read_pos..]) {
            Ok(nread) => {
                self.read_pos += nread;
                Ok(nread)
            },
            Err(e) => Err(e)
        }
    }

    fn shift_buffer(&mut self) {
        unsafe {
            let ptr = self.buf.as_mut_ptr();
            rlibc::memmove(ptr, self.buf[self.scan_pos..self.read_pos].as_ptr(), self.read_pos - self.scan_pos);
            self.read_pos -= self.scan_pos;
            self.scan_pos = 0;
        }
    }

    fn find_record<'a>(&mut self) -> io::Result<Option<&[u8]>> {
        let pos = (&self.buf[self.scan_pos..self.read_pos]).iter().position(|&x| x == RECORD_SEP);
        if pos.is_some() {
            let pos = pos.unwrap();
            self.offset += pos as u64;
            self.scan_pos += pos;
            if self.read_pos - self.scan_pos < 2 {
                return Ok(None);
            }

            let header_length = self.buf[self.scan_pos + 1] as usize;
            let header_end = self.scan_pos + header_length + 3;
            if header_end > self.read_pos {
                return Ok(None);
            }

            if self.header.has_message_length()
            || self.decode_header(header_end) {
                let message_end = header_end + self.header.get_message_length() as usize;
                if message_end > self.read_pos {
                    return Ok(None);
                }
                self.offset += (message_end - self.scan_pos) as u64;
                self.scan_pos = message_end;
                self.header.clear();
                return Ok(Some(&self.buf[header_end..message_end]));
            }
        } else {
            self.offset += (self.read_pos - self.scan_pos) as u64;
            self.scan_pos = self.read_pos;
        }
        Ok(None)
    }
}
