use std::io::Read;

use binrw::BinWrite;
use clap::Parser;
use soapy_spec_acc::{
    sigproc_io::Header,
    utils::{read_data, write_data},
};

#[derive(Debug, Parser)]
#[clap(author, about, version)]
struct Args {
    #[clap(short('f'), long("freq"), value_name("central freq in Hz"))]
    f0_Hz: f64,

    #[clap(short('n'), long("nch"), value_name("num of channels"))]
    nch: usize,

    #[clap(
        short('a'),
        value_name("number of time points to calculate mean"),
        default_value("1")
    )]
    n_average: usize,

    #[clap(short('s'), value_name("sampling rate in MHz"), default_value("6"))]
    sampling_rate: u32,

    #[clap(
        short('i'),
        long("in"),
        value_name("input raw"),
        //default_value("6")
    )]
    inname: String,

    #[clap(
        short('o'),
        long("out"),
        value_name("output filterbank file"),
        //default_value("6")
    )]
    outname: String,
}

pub fn main() -> Result<(), std::io::Error> {
    let args = Args::parse();
    let fs_MHz = args.sampling_rate as f64;
    println!("fs={:e}", fs_MHz);
    let nch = args.nch;
    let dt = 1.0 / (fs_MHz * 1e6) * nch as f64 / 2.0 * args.n_average as f64;
    let foff_MHz = -fs_MHz / nch as f64;

    println!("dt={} us", dt * 1e6);
    let fc_MHz = args.f0_Hz / 1e6;
    let fch1_MHz = fc_MHz + fs_MHz / 2.0 + foff_MHz / 2.0;
    println!("fch1: {} MHz", fch1_MHz);

    let header = Header::new(fch1_MHz, nch, foff_MHz, 51544.0, dt);
    let mut outfile = std::fs::File::create(&args.outname)?;

    header.write_le(&mut outfile).unwrap();

    let mut infile = std::fs::File::open(&args.inname)?;
    let mut buf = vec![0_f32; nch];
    let mut buf1 = vec![0_f32; nch];
    while let Ok(()) = read_data(&mut infile, &mut buf) {
        buf1.iter_mut().zip(buf.iter().rev()).for_each(|(a, &b)| {
            *a = b;
        });
        write_data(&mut outfile, &buf1);
    }
    Ok(())
}
