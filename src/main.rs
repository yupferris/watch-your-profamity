extern crate byteorder;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use std::collections::{HashMap, HashSet};
use std::io::{self, Cursor, Error, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

enum Command {
    NSCHello,
    NSCSMS,
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

impl<W: io::Write> WriteNtExt for W {}

struct User {
    name: String,
}

#[derive(Clone)]
struct Room {
    description: String,
    password: Option<String>,
    users: HashSet<String>,
}

struct ServerState {
    logged_in_users: HashMap<SocketAddr, User>,
    rooms: HashMap<String, Room>,
}

impl ServerState {
    fn new() -> ServerState {
        ServerState {
            logged_in_users: HashMap::new(),
            rooms: HashMap::new(),
        }
    }
}

fn chat_color(color: u32) -> String {
    format!("|c0{:06x}", color)
}

fn send_room_list(stream: &mut TcpStream, server_state: Arc<Mutex<ServerState>>) -> io::Result<()> {
    let rooms = server_state.lock().map_err(|e| Error::new(ErrorKind::Other, format!("{}", e)))?.rooms.clone();

    let mut response = EzPacketBuilder::new();
    response.write_u8(0x8c)?;
    response.write_u8(0x01)?;
    response.write_u8(0x01)?;
    response.write_u8(rooms.len() as _)?;
    for (name, room) in rooms.iter() {
        response.write_nt(name)?;
        response.write_nt(&room.description)?;
    }
    for _ in rooms.iter() {
        response.write_u8(0x00)?; // TODO: Proper status!!
    }
    for (_, room) in rooms.iter() {
        response.write_u8(if room.password.is_some() { 1 } else { 0 })?;
    }
    stream.write_all(&response.finish()?)
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
            0x0a => Command::NSCSMS,
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
                stream.write_all(&response.finish()?)?;
            }
            Command::NSCSMS => {
                let network_screen = packet.read_u8()?;
                match network_screen {
                    0x07 => {
                        // Entered ScreenNetRoom
                        send_room_list(stream, server_state.clone())?;
                    }
                    _ => {
                        println!("{}: Unrecognized network screen: 0x{:02x}", client_addr, network_screen);
                    }
                }
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
                                stream.write_all(&response.finish()?)?;

                                // Send welcome message(s)
                                let mut response = EzPacketBuilder::new();
                                response.write_u8(0x87)?;
                                response.write_nt(&format!("{}{}", chat_color(0x11ff11), welcome_message))?;
                                stream.write_all(&response.finish()?)?;
                                let mut response = EzPacketBuilder::new();
                                response.write_u8(0x87)?;
                                response.write_nt(&format!("{}{} | {}{}{} user(s) currently logged in", chat_color(0xffffff), server_name, chat_color(0xff0000), num_logged_in_users, chat_color(0xffffff)))?;
                                stream.write_all(&response.finish()?)?;

                                send_room_list(stream, server_state.clone())?;
                            }
                            Err(reason) => {
                                println!("{}: Client failed to log in as {}: {}", client_addr, username, reason);

                                // Send error response
                                let mut response = EzPacketBuilder::new();
                                response.write_u8(0x8c)?;
                                response.write_u8(command_byte)?;
                                response.write_u8(0x01)?;
                                response.write_nt(&reason)?;
                                stream.write_all(&response.finish()?)?;
                            }
                        }
                    }
                    2 => {
                        // New room
                        let room_name = payload.read_nt()?;
                        let room_description = payload.read_nt()?;
                        let room_password = if (payload.position() as usize) < payload.get_ref().len() {
                            Some(payload.read_nt()?)
                        } else {
                            None
                        };

                        let result = {
                            let mut server_state = server_state.lock().map_err(|e| Error::new(ErrorKind::Other, format!("{}", e)))?;
                            if server_state.rooms.contains_key(&room_name) {
                                Err(format!("Room {} already exists", room_name))
                            } else {
                                let user = server_state.logged_in_users.get(&client_addr).ok_or(Error::new(ErrorKind::Other, "Client is not logged in"))?;
                                let room = Room {
                                    description: room_description.clone(),
                                    password: room_password,
                                    users: vec![user.name.clone()].into_iter().collect(),
                                };
                                server_state.rooms.insert(room_name.clone(), room);
                                Ok(())
                            }
                        };

                        match result {
                            Ok(()) => {
                                println!("{}: Room {} created successfully", client_addr, room_name);

                                // Send success response
                                let mut response = EzPacketBuilder::new();
                                response.write_u8(0x87)?;
                                response.write_nt(&format!("{}Room {}{}{} created successfully", chat_color(0xffffff), chat_color(0x00ff00), room_name, chat_color(0xffffff)))?;
                                stream.write_all(&response.finish()?)?;

                                // Send user to room screen
                                let mut response = EzPacketBuilder::new();
                                response.write_u8(0x8c)?;
                                response.write_u8(0x01)?;
                                response.write_u8(0x00)?;
                                response.write_nt(&room_name)?;
                                response.write_nt(&room_description)?;
                                response.write_u8(0x01)?;
                                stream.write_all(&response.finish()?)?;

                                // TODO: Send room players
                                // TODO: Send join message to all room players
                            }
                            Err(reason) => {
                                println!("{}: Client failed to create room {}: {}", client_addr, room_name, reason);

                                // Send error response
                                let mut response = EzPacketBuilder::new();
                                response.write_u8(0x87)?;
                                response.write_nt(&format!("{}Failed to create room: {}", chat_color(0xaa0000), reason))?;
                                stream.write_all(&response.finish()?)?;
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
