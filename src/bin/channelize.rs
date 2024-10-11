use clap::Parser;

use chrono::prelude::*;
use ndarray::{Array1, Axis};
use num::complex::Complex;
use rsdsp::{ospfb2::Analyzer, windowed_fir::pfb_coeff};
use soapy_spec_acc::utils::send_data;
use soapysdr::{Device, Direction};
use std::net::UdpSocket;

use crossbeam::channel::bounded;

const SAMP_RATE: f64 = 6_000_000_f64;


type Ftype = f32;

#[derive(Debug, Parser)]
#[clap(author, about, version)]
struct Args {
    #[clap(short('f'), long("freq"), value_name("central freq in Hz"))]
    f0: f64,

    #[clap(
        short('n'),
        long("nch"),
        value_name("num of channels, must <=8192"),
        default_value("8192")
    )]
    nch: usize,

    #[clap(
        short('t'),
        long("tap"),
        value_name("pfb tap per ch"),
        default_value("8")
    )]
    ntap: usize,

    #[clap(
        short('m'),
        value_name("number of time points to calculate mean"),
        default_value("128")
    )]
    n_mean: usize,

    #[clap(short('P'), value_name("self udp port"), default_value("6665"))]
    tx_port: u16,

    #[clap(short('p'), value_name("receiver udp port"), default_value("6666"))]
    rx_port: u16,
}

//const ANTENNA:&str="RX";
fn main() {
    let args = Args::parse();
    assert_eq!(args.nch & (args.nch - 1), 0);

    let coeff = pfb_coeff::<Ftype>(args.nch / 2, args.ntap, 1.1 as Ftype);
    let mut pfb = Analyzer::<Complex<Ftype>, Ftype>::new(args.nch, coeff.as_slice().unwrap());

    let device = Device::new("driver=airspy").unwrap();

    for g in device.list_gains(Direction::Rx, 0).unwrap() {
        println!("{}", g);
    }

    device.set_antenna(Direction::Rx, 0, "RX").unwrap();
    device.set_sample_rate(Direction::Rx, 0, SAMP_RATE).unwrap();
    device
        .set_gain_element(Direction::Rx, 0, "LNA", 10.0)
        .unwrap();
    device
        .set_gain_element(Direction::Rx, 0, "MIX", 5.0)
        .unwrap();
    device
        .set_gain_element(Direction::Rx, 0, "VGA", 8.0)
        .unwrap();

    device.set_frequency(Direction::Rx, 0, args.f0, ()).unwrap();
    let mut stream = device.rx_stream::<Complex<Ftype>>(&[0]).unwrap();
    stream.activate(None).expect("failed to activate stream");

    //let sb = signalbool::SignalBool::new(&[signalbool::Signal::SIGINT], signalbool::Flag::Restart).unwrap();

    //let mut num=12_000_000;
    //let read_size = min(num as usize, buf.len());
    let mut num = 0;
    let mut cnt = 0;

    let t0 = Utc::now().timestamp_millis(); // e.g. `2014-11-28T12:45:59.324310806Z`
    let (tx_raw, rx_raw) = bounded(32);
    let (tx_spectrum, rx_spectrum) = bounded(32);
    std::thread::spawn(move || loop {
        let data: Vec<Complex<f32>> = rx_raw.recv().unwrap();
        pfb.analyze_raw_par(&data).axis_iter(Axis(0)).for_each(|x| {
            tx_spectrum.send(x.map(|x1| x1.norm_sqr())).unwrap();
        });
    });

    let addr = format!("127.0.0.1:{}", args.rx_port);

    std::thread::spawn(move || {
        //let mut filtered_result=Array1::<Ftype>::zeros(NCH);
        //let mut outfile=File::create("./a.bin").unwrap();
        let udp = UdpSocket::bind(format!("127.0.0.1:{}", args.tx_port)).unwrap();
        loop {
            let mut temp = Array1::<Ftype>::zeros(args.nch);
            for _i in 0..args.n_mean {
                temp = temp + rx_spectrum.recv().unwrap();
            }
            temp = temp / args.n_mean as Ftype;
            //filtered_result=filtered_result*K+temp*(1 as Ftype-K);
            send_data(&udp, temp.as_slice().unwrap(), &addr);
            //write_data(&mut outfile, filtered_result.as_slice().unwrap());
        }
    });

    loop {
        let mut buf = vec![Complex::<Ftype>::default(); stream.mtu().unwrap()];

        let len = stream
            .read(&mut [&mut buf], 1_000_000)
            .expect("read failed");
        buf.resize(len, Complex::default());
        tx_raw.send(buf).unwrap();

        //pfb.analyze_par(&buf[..len]);
        cnt += 1;
        num += len as i64;
        //println!("{}", num);
        //println!("{}", len);
        if cnt % 100 == 0 {
            let t1 = Utc::now().timestamp_millis();
            let dt_sec = (t1 - t0) as f64 / 1000.0;
            let sps = num as f64 / dt_sec;
            println!("{} Msps {}", sps / 1e6, tx_raw.len());
        }
    }
    //stream.deactivate(None).expect("failed to deactivate");
}
