use std::{io::Write, net::{ToSocketAddrs, UdpSocket}};

pub fn write_data<T: Sized + Default + Clone, W: Write>(drain: &mut W, buf: &[T]) {
    let buf = unsafe {
        std::slice::from_raw_parts(buf.as_ptr() as *const u8, std::mem::size_of_val(buf))
    };
    drain.write_all(buf).unwrap();
}


pub fn send_data<T: Sized+Default+Clone, A: ToSocketAddrs>(socket: &UdpSocket, buf: &[T], addr: A){
    let buf = unsafe {
        std::slice::from_raw_parts(buf.as_ptr() as *const u8, std::mem::size_of_val(buf))
    };
    socket.send_to(buf, addr).unwrap();
}
