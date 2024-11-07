use std::{
    io::{Read, Write},
    net::{ToSocketAddrs, UdpSocket},
};

pub fn write_data<T: Sized + Default + Clone, W: Write>(drain: &mut W, buf: &[T]) {
    let buf = unsafe {
        std::slice::from_raw_parts(buf.as_ptr() as *const u8, std::mem::size_of_val(buf))
    };
    drain.write_all(buf).unwrap();
}

pub fn read_data<T: Sized + Default + Clone, R: Read>(source: &mut R, buf: &mut [T]) -> Result<(), std::io::Error>{
    let buf = unsafe {
        std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, std::mem::size_of_val(buf))
    };
    source.read_exact(buf)
}

pub fn send_data<T: Sized + Default + Clone, A: ToSocketAddrs>(
    socket: &UdpSocket,
    buf: &[T],
    addr: A,
) {
    let buf = unsafe {
        std::slice::from_raw_parts(buf.as_ptr() as *const u8, std::mem::size_of_val(buf))
    };
    socket.send_to(buf, addr).unwrap();
}
