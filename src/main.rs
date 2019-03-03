extern crate byteorder;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use std::fs::File;
use std::io::{self, Cursor, Error, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;

enum Command {
    NSCHello,
    NSSMONL,
}

struct EzPacketBuilder {
    buffer: Vec<u8>,
}

impl EzPacketBuilder {
    fn new() -> EzPacketBuilder {
        EzPacketBuilder {
            buffer: Vec::new(),
        }
    }

    fn write_nt(&mut self, s: &str) -> io::Result<()> {
        write!(&mut self.buffer, "{}\0", s)
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

impl<R: io::Read + ?Sized> ReadNtExt for R {}

fn thread_proc(stream: &mut TcpStream, server_name: &str, client_addr: SocketAddr) -> io::Result<()> {
    loop {
        // Read packet
        let size = stream.read_u32::<BigEndian>()?;
        //println!("Packet size read: {:?}", size);
        let mut packet_buf = vec![0; size as usize];
        if stream.read(&mut packet_buf)? != size as usize {
            println!("{}: Couldn't read entire packet :(", client_addr);
            continue; // TODO: lolzorz
        }
        let mut packet = Cursor::new(packet_buf);
        let command_byte = packet.read_u8()?;
        //println!("Got command byte: {:02x}", command_byte);
        let command = match command_byte {
            0x02 => Command::NSCHello,
            0x0c => Command::NSSMONL,
            x => {
                println!("{}: Unknown command byte: {:02x}", client_addr, x);
                continue; // TODO: lolzorz
            }
        };
        match command {
            Command::NSCHello => {
                // Read protocol version
                let protocol_version = packet.read_u8()?;
                // Read product ID
                let product_id = packet.read_nt()?;
                println!("{}: Received handshake using client \"{}\" and protocol version {}", client_addr, product_id, protocol_version);

                // Send response
                let mut response = EzPacketBuilder::new();
                response.write_u8(0x82)?;
                response.write_u8(0x80)?;
                response.write_nt(server_name)?;
                stream.write(&response.finish()?)?;
            }
            Command::NSSMONL => {
                let payload = &packet.get_ref()[packet.position() as usize..];
                let dump_file_name = "derp.bin";
                {
                    let mut file = File::create(dump_file_name)?;
                    file.write(payload)?;
                }
                println!("{}: Got NSSMONL packet; payload dumped to {}", client_addr, dump_file_name);
            }
        }
    }
}

fn main() -> io::Result<()> {
    let server_name = "Super Best Server 2k19 Jake Rules c==3";
    let ipv4_addr = "127.0.0.1";
    let port = 8765;

    println!("{}", server_name);

    let listener = TcpListener::bind(format!("{}:{}", ipv4_addr, port))?;

    println!("Listening for incoming connections on {}:{}", ipv4_addr, port);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let client_addr = stream.peer_addr()?;
                println!("{}: Client connected", client_addr);

                thread::spawn(move || {
                    if let Err(e) = thread_proc(&mut stream, server_name, client_addr) {
                        println!("{}: Client errored: {}; connection dropped", client_addr, e);
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
