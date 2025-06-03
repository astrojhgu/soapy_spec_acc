use async_stream::stream;

use futures_util::pin_mut;
use futures_util::stream::StreamExt;

use clap::Parser;

use chrono::prelude::*;

use ndarray::{Array1, Axis, s};

//use rayon::prelude::*;

use num::complex::Complex;
use rsdsp::{ospfb2::Analyzer, windowed_fir::pfb_coeff};
use soapysdr::{Device, Direction};

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
        default_value("512")
    )]
    nch: usize,

    #[clap(
        short('t'),
        long("tap"),
        value_name("pfb tap per ch"),
        default_value("4")
    )]
    ntap: usize,

    #[clap(
        short('y'),
        value_name("num of time points displayed"),
        default_value("128")
    )]
    ntime: usize,

    #[clap(short('k'), value_name("filter param k"), default_value("0.9"))]
    k: f32,

    #[clap(
        short('a'),
        value_name("number of time points to calculate mean"),
        default_value("128")
    )]
    n_average: usize,

    #[clap(long("lna"), value_name("lna gain"), default_value("5"))]
    lna: f64,

    #[clap(long("mix"), value_name("mix gain"), default_value("5"))]
    mix: f64,

    #[clap(long("vga"), value_name("vga gain"), default_value("5"))]
    vga: f64,

    #[clap(short('s'), value_name("sampling rate in MHz"), default_value("6"))]
    sampling_rate: u32,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.sampling_rate != 3 && args.sampling_rate != 6 {
        eprintln!("Sampling rate can only be either 3 or 6 MSps");
        return;
    }

    let sampling_rate = args.sampling_rate as f64 * 1e6;
    assert_eq!(args.nch & (args.nch - 1), 0);

    let coeff = pfb_coeff::<Ftype>(args.nch / 2, args.ntap, 1.1 as Ftype);
    let mut pfb = Analyzer::<Complex<Ftype>, Ftype>::new(args.nch, coeff.as_slice().unwrap());

    let device = Device::new("driver=airspy").unwrap();

    for g in device.list_gains(Direction::Rx, 0).unwrap() {
        println!("{g}");
    }

    device.set_antenna(Direction::Rx, 0, "RX").unwrap();
    device
        .set_sample_rate(Direction::Rx, 0, sampling_rate)
        .unwrap();
    device
        .set_gain_element(Direction::Rx, 0, "LNA", args.lna)
        .unwrap();
    device
        .set_gain_element(Direction::Rx, 0, "MIX", args.mix)
        .unwrap();
    device
        .set_gain_element(Direction::Rx, 0, "VGA", args.vga)
        .unwrap();

    device.set_frequency(Direction::Rx, 0, args.f0, ()).unwrap();
    let mut sdr_stream = device.rx_stream::<Complex<Ftype>>(&[0]).unwrap();
    sdr_stream
        .activate(None)
        .expect("failed to activate stream");

    let mut num = 0;
    let mut cnt = 0;

    let t0 = Utc::now().timestamp_millis(); // e.g. `2014-11-28T12:45:59.324310806Z`
    let daq_stream = stream! {
        loop{
            let mut buf = Vec::with_capacity(sdr_stream.mtu().unwrap());
            buf.resize(sdr_stream.mtu().unwrap(), Complex::default());
            match sdr_stream
                .read(&mut [&mut buf], 1_000_000)
            {
                Ok(len)=>{
                buf.resize(len, Complex::default());
                yield buf;
                cnt += 1;
                num += len as i64;
                //println!("{}", num);
                //println!("{}", len);
                if cnt % 100 == 0 {
                    let t1 = Utc::now().timestamp_millis();
                    let dt_sec = (t1 - t0) as f64 / 1000.0;
                    let sps = num as f64 / dt_sec;
                    println!("{} Msps", sps / 1e6);
                }
                }
                otherwise =>{
                    println!("{otherwise:?}");
                }
            }

            //pfb.analyze_par(&buf[..len]);

        }
    };
    pin_mut!(daq_stream); // needed for iteration

    let spectrum_stream = stream! {
        loop{
            let raw_data=daq_stream.next().await.unwrap();
            for x in pfb.analyze_raw_par(&raw_data).axis_iter(Axis(0)){
                let x1 = Array1::from_iter(
                    x.slice(s![args.nch / 2..args.nch])
                        .iter()
                        .chain(x.slice(s![0..args.nch / 2]))
                        .map(|x1| x1.norm_sqr()),
                );
                yield x1;
            }
            ;
        }
    };
    pin_mut!(spectrum_stream);

    let average_stream = stream! {
        loop {
            let mut temp = Array1::<Ftype>::zeros(args.nch);
            for _i in 0..args.n_average {
                //temp = temp + //rx_spectrum.recv().unwrap();
                temp=temp+spectrum_stream.next().await.unwrap();
            }
            temp /= args.n_average as Ftype;
            if temp.iter().all(|&x| x>0_f32){
                yield temp;
            }
            }
    };

    pin_mut!(average_stream);

    while let Some(_x) = average_stream.next().await {
        //println!("{}", x.len());
    }
}
