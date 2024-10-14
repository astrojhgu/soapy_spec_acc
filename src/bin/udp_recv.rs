use std::net::UdpSocket;

pub fn main() {
    let socket = UdpSocket::bind("127.0.0.1:7777").unwrap();
    let mut data = vec![0_u8; 32768];
    println!("{:?}", socket.recv_from(&mut data));
}
