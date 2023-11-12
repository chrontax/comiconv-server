use comiconv::*;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    thread::spawn,
    time::Duration,
};

fn main() {
    let mut args = std::env::args();
    args.next();
    let mut port = 2137;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }
            "-v" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "-p" | "--port" => port = args.next().unwrap().parse().unwrap(),
            _ => {
                println!("Unknown argument: {}", arg);
                return;
            }
        }
    }
    let listener = TcpListener::bind(("0.0.0.0", port)).unwrap();
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        stream.set_nodelay(true).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        spawn(move || {
            handle_client(stream);
        });
    }
}

fn handle_client(mut stream: TcpStream) {
    {
        let mut buf = [0; 4];
        if stream.read_exact(&mut buf).is_err() {
            stream.shutdown(Shutdown::Both).unwrap();
            return;
        }
        if &buf != b"comi" {
            return;
        }
    }
    stream.write_all(b"conv").unwrap();
    let conv;
    let len;
    {
        let mut buf = [0; 8];
        stream.read_exact(&mut buf).unwrap();
        let format = match &buf[0] {
            b'J' => Format::Jpeg,
            b'P' => Format::Png,
            b'W' => Format::Webp,
            b'A' => Format::Avif,
            _ => return,
        };
        let quality = buf[1].clamp(0, 100);
        let speed = buf[2].clamp(0, 10);
        conv = Converter {
            format,
            quality,
            speed,
            quiet: true,
            backup: false,
        };
        len = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    }
    let hash = {
        let mut buf = [0; 32];
        stream.read_exact(&mut buf).unwrap();
        buf
    };
    let mut file = Vec::with_capacity(len);
    let mut left = len;
    while left > 0 {
        let mut buf = vec![0; left.min(1024 * 1024)];
        stream.read_exact(&mut buf).unwrap();
        stream.write_all(b"ok").unwrap();
        file.extend_from_slice(&buf);
        left -= left.min(1024 * 1024);
    }
    let mut hasher = Sha256::new();
    hasher.update(&file);
    if hasher.finalize() != hash.into() {
        return;
    }
    let file = conv.convert(&file, Some(&mut stream));
    let len = file.len() as u32;
    let hash = {
        let mut hasher = Sha256::new();
        hasher.update(&file);
        hasher.finalize()
    };
    stream.write_all(&len.to_be_bytes()).unwrap();
    stream.write_all(&hash).unwrap();
    stream.write_all(&file).unwrap();
    handle_client(stream);
}

fn print_help() {
    println!("Usage: comiconv-server [options]");
    println!();
    println!("Options:");
    println!();
    println!("  -h, --help\t\tPrint this help message");
    println!("  -v, --version\t\tPrint version");
    println!("  -p, --port\t\tPort to listen on (default: 2137)");
}
