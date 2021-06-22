use std::io::{self, Read, Write,};
use std::net::{TcpStream, Ipv4Addr};
use std::thread;
use std::net::Shutdown;

/// Represent's a `Socks4` packet structure
/// 
///
///              +----+----+----+----+----+----+----+----+----+----+....+----+
///              | VN | CD | DSTPORT |      DSTIP        | USERID       |NULL|
///              +----+----+----+----+----+----+----+----+----+----+....+----+
///  # of bytes:	   1    1      2              4           variable       1
pub struct Sock4Request {
    version_number: u8,
    command: u8,
    dst_port: u16,
    dst_ip: Ipv4Addr,
    userid: Option<String>,
}

/// `Sock4Reply` packet is sent to the client when one of the following 
/// occured. `Connection Established` or `Request Rejected` and 
/// 'Operation Failed' with reply_code `Socks4ReplyCode`.
///
/// Sock4Reply packet on wire
///             +----+----+----+----+----+----+----+----+
///	            | VN | CD | DSTPORT |      DSTIP        |
///	            +----+----+----+----+----+----+----+----+
/// # of bytes:	   1    1      2              4
pub struct Sock4Reply {
    version_number: u8,
    reply_code: Socks4ReplyCode,
    dst_port: u16,
    dst_ip: u32,
}

/// sock4 reply codes
pub enum Socks4ReplyCode {
    RequestGranted = 0x5A,   // Granted
    RequestFailed = 0x5B,    // Rejected or Failed
    RequestRejected = 0x5C,  // Rejected <because socks server could not connect to identd on the client>
    RequestRejedtedB = 0x5D, // Rejected <because the client program and identd reported different user-ids>
}


impl Sock4Reply {
    /// serialize `Sock4Reply` struct into to byte streams.
    ///
    /// returns `()` on success and `io::Error` on error
    pub fn serialize(self, mut buffer: impl Write) -> io::Result<()> {
        // since we control the struct and at the stage we 
        // know all values are provided
        buffer.write(&[self.version_number, self.reply_code as u8])?;
        buffer.write(&self.dst_port.to_ne_bytes())?;
        buffer.write(&self.dst_ip.to_ne_bytes())?;
        Ok(())
    }
}

impl Sock4Request {
    /// deserialize packet into a `Sock4Request` struct.
    ///
    pub fn deserialize<R: Read>(stream: &mut R) -> io::Result<Self> {
        let mut version = [0u8; 1];
        let mut command = [0u8; 1];
        let mut dst_port = [0u8; 2];
        let mut dst_ip = [0u8; 4];
        let mut userid = [0u8; 255];

        // TODO: proper error handling for malformed socks4 packet
        stream.read_exact(version.as_mut())?;
        stream.read_exact(command.as_mut())?;
        stream.read_exact(dst_port.as_mut())?;
        stream.read_exact(dst_ip.as_mut())?;

        // using a max of 255 of the username buf length
        stream.read(userid.as_mut())?;
        
        Ok(Sock4Request {
            version_number: 0x04,
            command: command[0],
            dst_port: u16::from_be_bytes(dst_port),
            dst_ip: Ipv4Addr::from(dst_ip),
            // TODO: implement identd support
            userid: None,
        })
    }
}


pub fn handle_sock4_client(req: &mut Sock4Request, stream: &mut TcpStream) -> io::Result<()> {
    // TODO: sock4 only support basic request, sock4 `username field will be use when` implementing
    // ident support. for now ignoring auth support.
    let target: TcpStream = TcpStream::connect((req.dst_ip, req.dst_port))?;
    debug!("Connected to destination host");
    // if no error connecting to the stream, send a reply packet to the client
    // TODO: pack into a Sock4Reply reply struct and pass to write_all Read function
    // also proper error handling coming. would need to handlee `connect` error after
    // replying to the client as specified on Socks4 Specification.
    stream.write_all(&[0x00, 0x5A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])?;

    // using @ajmwagar's code, will update and uptimize when moving to async rust
    // Copy it all
    let mut outbound_in = target.try_clone()?;
    let mut outbound_out = target.try_clone()?;
    let mut inbound_in = stream.try_clone()?;
    let mut inbound_out = stream.try_clone()?;


    // Upload Thread
    thread::spawn(move || {
        io::copy(&mut inbound_in, &mut outbound_out).is_ok();
        inbound_in.shutdown(Shutdown::Read).unwrap_or(());
        outbound_out.shutdown(Shutdown::Write).unwrap_or(());
    });

    // Download Thread
    thread::spawn(move || {
        io::copy(&mut outbound_in, &mut inbound_out).is_ok();
        outbound_in.shutdown(Shutdown::Read).unwrap_or(());
        inbound_out.shutdown(Shutdown::Write).unwrap_or(());
    });


    Ok(())
}

