extern crate byteorder;

use byteorder::{ReadBytesExt, BigEndian};

use std::io::{Cursor, Error, ErrorKind, Read, Write};
use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    {
        let listener = TcpListener::bind("127.0.0.1:8765")?;

        for stream in listener.incoming() {
            let mut stream = stream?;

            println!("Client connected: {:?}", stream);

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
                }
                let command = match command_byte {
                    0x02 => Command::Hello,
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
                        let response = [0, 0, 0, 5, 0x82, 3, 72, 105, 0];
                        stream.write(&response)?;
                    }
                }
            }
        }
    }
    Ok(())
}
