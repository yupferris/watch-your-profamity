extern crate byteorder;

use byteorder::{ReadBytesExt, BigEndian};

use std::fs::File;
use std::io::{self, Cursor, Error, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn thread_proc(stream: &mut TcpStream) -> io::Result<()> {
    loop {
        // Read packet
        let size = stream.read_u32::<BigEndian>()?;
        println!("Packet size read: {:?}", size);
        let mut packet_buf = vec![0; size as usize];
        if stream.read(&mut packet_buf)? != size as usize {
            println!("Couldn't read entire packet :(");
            continue; // TODO: lolzorz
        }
        let mut packet_cursor = Cursor::new(packet_buf);
        let command_byte = packet_cursor.read_u8()?;
        println!("Got command byte: {:02x}", command_byte);
        enum Command {
            Hello,
            SMOnline,
        }
        let command = match command_byte {
            0x02 => Command::Hello,
            0x0c => Command::SMOnline,
            x => {
                println!("Unknown command byte: {:02x}", x);
                continue; // TODO: lolzorz
            }
        };
        match command {
            Command::Hello => {
                // Read protocol version
                let protocol_version = packet_cursor.read_u8()?;
                println!("Read protocol version: {:02x}", protocol_version);
                // Read product ID
                let product_id = std::str::from_utf8(&packet_cursor.get_ref()[packet_cursor.position() as usize..]).map_err(|e| Error::new(ErrorKind::Other, e))?;
                println!("Read product ID: {}", product_id);

                // Send response
                let response = [0, 0, 0, 5, 0x82, 0x80, 72, 105, 0];
                stream.write(&response)?;
            }
            Command::SMOnline => {
                let payload = &packet_cursor.get_ref()[packet_cursor.position() as usize..];
                let dump_file_name = "derp.bin";
                {
                    let mut file = File::create(dump_file_name)?;
                    file.write(payload)?;
                }
                println!("Got SMOnline packet; payload dumped to {}", dump_file_name);
            }
        }
    }
}

fn main() -> io::Result<()> {
    let ipv4_addr = "127.0.0.1";
    let port = 8765;

    let listener = TcpListener::bind(format!("{}:{}", ipv4_addr, port))?;

    println!("Listening for incoming connections on {}:{}", ipv4_addr, port);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                println!("Client connected: {:?}", stream);

                thread::spawn(move || {
                    if let Err(e) = thread_proc(&mut stream) {
                        println!("Client {:?} errored ({}); connection dropped", stream, e);
                    }
                });
            }
            Err(e) => {
                println!("Client connection failed: {}", e);
            }
        }
    }

    Ok(())
}
