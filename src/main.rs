//use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use tokio::io;
use tokio::net::TcpListener;
use tokio::prelude::*;

use std::io::{Error, ErrorKind};

/*use std::collections::{HashMap, HashSet};
use std::io::{self, Cursor, Error, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;*/

/*struct EzPacketBuilder {
    buffer: Vec<u8>,
}

impl EzPacketBuilder {
    fn new() -> EzPacketBuilder {
        EzPacketBuilder {
            buffer: Vec::new(),
        }
    }

    fn finish(self) -> io::Result<Vec<u8>> {
        let mut ret = Vec::with_capacity(4 + self.buffer.len());
        ret.write_u32::<BigEndian>(self.buffer.len() as _)?;
        ret.extend(self.buffer);
        Ok(ret)
    }
}

impl Write for EzPacketBuilder {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buffer.flush()
    }
}

trait ReadNtExt: io::Read {
    fn read_nt(&mut self) -> io::Result<String> {
        let mut buf = Vec::new();
        loop {
            let b = self.read_u8()?;
            if b == 0x00 {
                break;
            }
            buf.push(b);
        }
        String::from_utf8(buf).map_err(|e| Error::new(ErrorKind::Other, e))
    }
}

impl<R: io::Read> ReadNtExt for R {}

trait WriteNtExt: io::Write {
    fn write_nt(&mut self, s: &str) -> io::Result<()> {
        write!(self, "{}\0", s)
    }
}

impl<W: io::Write> WriteNtExt for W {}*/

fn main() -> io::Result<()> {
    let server_name = "Super Best Server 2k19 Jake Rules c==3";
    let welcome_message = "Sup weeb";
    let ipv4_addr = "127.0.0.1";
    let port = 8765;

    println!("{}", server_name);

    let addr = format!("{}:{}", ipv4_addr, port).parse().map_err(|e| Error::new(ErrorKind::Other, e))?;
    let listener = TcpListener::bind(&addr)?;

    println!("Listening for incoming connections on {}:{}", ipv4_addr, port);

    let server =
        listener.incoming()
        .map_err(|e| eprintln!("Client connection failed: {}", e))
        .for_each(|stream| {
            let client_addr = stream.peer_addr().unwrap(); // TODO!!!!!
            println!("{}: Client connected", client_addr);

            let (reader, _) = stream.split();

            let reader =
                reader
                .for_each(|bytes| {
                    println!("{}: Got some bytes: {:?}", client_addr, bytes);
                    Ok(())
                })
                .and_then(|()| {
                    println!("{}: Client closed connection", client_addr);
                    Ok(())
                })
                .or_else(|err| {
                    println!("{}: Socket closed with error: {:?}", client_addr, err);
                    Err(err)
                })
                .then(|result| {
                    println!("{}: Socket closed with result: {:?}", client_addr, result);
                    Ok(())
                });

            tokio::spawn(reader);
        });

    /*for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let client_addr = stream.peer_addr()?;
                println!("{}: Client connected", client_addr);

                let server_state = server_state.clone();
                thread::spawn(move || {
                    if let Err(e) = thread_loop(&mut stream, server_state.clone(), server_name, welcome_message, client_addr) {
                        println!("{}: Client errored: {}; connection dropped", client_addr, e);
                        // Log out logged in user for this connection, if any
                        match server_state.lock() {
                            Ok(mut server_state) => {
                                if let Some(user) = server_state.logged_in_users.remove(&client_addr) {
                                    println!("{}: Client was previously logged in as {}; logged out", client_addr, user.name);
                                }
                            }
                            Err(e) => {
                                println!("{}: Failed to acquire lock for server state: {}; state might be inconsistent", client_addr, e);
                            }
                        }
                    }
                });
            }
            Err(e) => {
                println!("Client connection failed: {}", e);
            }
        }
    }*/

    tokio::run(server);

    Ok(())
}
