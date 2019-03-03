extern crate byteorder;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use std::collections::HashMap;
use std::io::{self, Cursor, Error, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
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

struct User {
    name: String,
}

struct ServerState {
    logged_in_users: HashMap<SocketAddr, User>,
}

impl ServerState {
    fn new() -> ServerState {
        ServerState {
            logged_in_users: HashMap::new(),
        }
    }
}

fn thread_proc(stream: &mut TcpStream, server_state: Arc<Mutex<ServerState>>, server_name: &str, welcome_message: &str, client_addr: SocketAddr) -> io::Result<()> {
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
                println!("{}: Unknown command byte: 0x{:02x}", client_addr, x);
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
                // TODO: Probably don't need to copy here, but keeps things easy..
                let mut payload = Cursor::new(packet.get_ref()[packet.position() as usize..].to_vec());
                let command_byte = payload.read_u8()?;
                let command_sub_byte = payload.read_u8()?;
                match command_byte {
                    0 => {
                        // Login
                        let _ = payload.read_u8()?; // Reserved

                        let username = payload.read_nt()?;
                        let password_hash = payload.read_nt()?;
                        println!("{}: Received login for user {} with password hash {}", client_addr, username, password_hash);

                        let logged_out_user;
                        let result = {
                            let mut server_state = server_state.lock().map_err(|e| Error::new(ErrorKind::Other, format!("{}", e)))?;
                            logged_out_user = server_state.logged_in_users.remove(&client_addr);
                            if let Some(user) = server_state.logged_in_users.values().find(|user| user.name == username) {
                                Err(format!("{} is already logged in", user.name))
                            } else {
                                // TODO: Match password
                                server_state.logged_in_users.insert(client_addr, User {
                                    name: username.clone(),
                                });
                                Ok(server_state.logged_in_users.len())
                            }
                        };

                        if let Some(user) = logged_out_user {
                            println!("{}: Client was previously logged in as {}; logged out", client_addr, user.name);
                        }

                        match result {
                            Ok(num_logged_in_users) => {
                                println!("{}: Client succesfully logged in as {}", client_addr, username);

                                // Send success response
                                let mut response = EzPacketBuilder::new();
                                response.write_u8(0x8c)?;
                                response.write_u8(command_byte)?;
                                response.write_u8(command_sub_byte)?;
                                response.write_nt("Login successful yo!!!! Git sum")?;
                                stream.write(&response.finish()?)?;

                                // Send welcome message(s)
                                let mut response = EzPacketBuilder::new();
                                response.write_u8(0x87)?;
                                response.write_nt(welcome_message)?;
                                stream.write(&response.finish()?)?;
                                let mut response = EzPacketBuilder::new();
                                response.write_u8(0x87)?;
                                response.write_nt(&format!("{} | {} user(s) currently logged in", server_name, num_logged_in_users))?;
                                stream.write(&response.finish()?)?;
                            }
                            Err(reason) => {
                                println!("{}: Client failed to log in as {}: {}", client_addr, username, reason);

                                // Send error response
                                let mut response = EzPacketBuilder::new();
                                response.write_u8(0x8c)?;
                                response.write_u8(command_byte)?;
                                response.write_u8(0x01)?;
                                response.write_nt(&reason)?;
                                stream.write(&response.finish()?)?;
                            }
                        }
                    }
                    _ => {
                        println!("{}: Unrecognized SMOnline command: 0x{:02x} 0x{:02x}", client_addr, command_byte, command_sub_byte);
                    }
                }
            }
        }
    }
}

fn main() -> io::Result<()> {
    let server_name = "Super Best Server 2k19 Jake Rules c==3";
    let welcome_message = "Sup weeb";
    let ipv4_addr = "127.0.0.1";
    let port = 8765;

    println!("{}", server_name);

    let listener = TcpListener::bind(format!("{}:{}", ipv4_addr, port))?;

    println!("Listening for incoming connections on {}:{}", ipv4_addr, port);

    let server_state = Arc::new(Mutex::new(ServerState::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let client_addr = stream.peer_addr()?;
                println!("{}: Client connected", client_addr);

                let server_state = server_state.clone();
                thread::spawn(move || {
                    if let Err(e) = thread_proc(&mut stream, server_state.clone(), server_name, welcome_message, client_addr) {
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
    }

    Ok(())
}
