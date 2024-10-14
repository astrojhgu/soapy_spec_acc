use std::net::UdpSocket;

pub fn main() {
    let socket = UdpSocket::bind("127.0.0.1:6666").unwrap();
    let data = vec![0_u8; 32768];
    socket.send_to(&data, "127.0.0.1:8888").unwrap();
}
