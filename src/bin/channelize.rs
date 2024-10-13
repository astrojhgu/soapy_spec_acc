use clap::Parser;

use chrono::prelude::*;
use egui::Vec2;
use image::{imageops::FilterType::Nearest, DynamicImage, RgbImage};
use ndarray::{s, Array1, Array2, Axis};

//use rayon::prelude::*;

use num::complex::Complex;
use rsdsp::{ospfb2::Analyzer, windowed_fir::pfb_coeff};
use soapysdr::{Device, Direction};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, CentralPanel, Context, Visuals};
use egui_plotter::EguiBackend;
use plotters::prelude::*;

use crossbeam::channel::bounded;

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

#[derive(Clone, Copy)]
struct PlotSpec {
    fmin: f64,
    fmax: f64,
    ntime: usize,
    nch: usize,
}

fn db(x: f64) -> f64 {
    x.log10() * 10.0
}

//const ANTENNA:&str="RX";
fn main() {
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
        println!("{}", g);
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
    let mut stream = device.rx_stream::<Complex<Ftype>>(&[0]).unwrap();
    stream.activate(None).expect("failed to activate stream");

    //let sb = signalbool::SignalBool::new(&[signalbool::Signal::SIGINT], signalbool::Flag::Restart).unwrap();

    //let mut num=12_000_000;
    //let read_size = min(num as usize, buf.len());
    let mut num = 0;
    let mut cnt = 0;

    let t0 = Utc::now().timestamp_millis(); // e.g. `2014-11-28T12:45:59.324310806Z`
    let (tx_raw, rx_raw) = bounded(64);
    let (tx_spectrum, rx_spectrum) = bounded(args.n_average * 2);
    let _th_channelize = std::thread::spawn(move || loop {
        let data: Vec<Complex<f32>> = rx_raw.recv().unwrap();
        pfb.analyze_raw_par(&data).axis_iter(Axis(0)).for_each(|x| {
            let x1 = Array1::from_iter(
                x.slice(s![args.nch / 2..args.nch])
                    .iter()
                    .chain(x.slice(s![0..args.nch / 2]))
                    .map(|x1| x1.norm_sqr()),
            );
            if !tx_spectrum.is_full() {
                tx_spectrum.send(x1).unwrap();
            } else {
                println!("WARNING: spectrum queue is full, skipping");
            }
        });
    });

    let (tx_averaged, rx_averaged) = bounded(16);

    let _th_filter = std::thread::spawn(move || {
        //let mut filtered_result=Array1::<Ftype>::zeros(NCH);
        //let mut outfile=File::create("./a.bin").unwrap();

        //let udp = UdpSocket::bind(format!("127.0.0.1:{}", args.tx_port)).unwrap();
        loop {
            let mut temp = Array1::<Ftype>::zeros(args.nch);
            for _i in 0..args.n_average {
                temp = temp + rx_spectrum.recv().unwrap();
            }
            temp /= args.n_average as Ftype;

            //filtered_result=filtered_result*K+temp*(1 as Ftype-K);
            //send_data(&udp, temp.as_slice().unwrap(), &addr);
            //write_data(&mut outfile, filtered_result.as_slice().unwrap());

            if !tx_averaged.is_full() && temp.iter().all(|&x| x > 0_f32) {
                tx_averaged.send(temp).unwrap();
            } else {
                println!("average data queue full, skipping");
            }
        }
    });

    let ctx = Arc::new(Mutex::new(Option::<Context>::default()));
    let ctx1 = Arc::clone(&ctx);

    //let waterfall_img_buf = Arc::new(Mutex::new(vec![0_u8; (args.ntime * args.nch * 3)]));
    let waterfall_img_buf = Arc::new(Mutex::new(Array2::<f32>::zeros((args.ntime, args.nch))));
    let spectrum_buf = Arc::new(Mutex::new(Array1::<f32>::zeros(args.nch)));

    let wimg = waterfall_img_buf.clone();
    let sbuf = spectrum_buf.clone();

    let (tx_repaint, rx_repaint) = bounded(1);

    let _th_display = std::thread::spawn(move || {
        let spectrum_buf = sbuf;

        let mut waterfall_buf = Array2::<f32>::zeros((args.ntime, args.nch));
        let mut waterfall_buf_tmp = Array2::<f32>::zeros((args.ntime, args.nch));
        //let averaged = rx_averaged.recv().unwrap();
        //let mut filtered_result = averaged.clone();
        let mut filtered_result = Array1::<f32>::zeros(args.nch);
        loop {
            let averaged = rx_averaged.recv().unwrap();
            filtered_result = filtered_result * args.k + &averaged * (1 as Ftype - args.k);
            waterfall_buf_tmp
                .slice_mut(s![..-1, ..])
                .assign(&waterfall_buf.slice(s![1.., ..]));
            waterfall_buf_tmp.slice_mut(s![-1, ..]).assign(&averaged);
            std::mem::swap(&mut waterfall_buf, &mut waterfall_buf_tmp);

            {
                if let Ok(mut g) = spectrum_buf.try_lock() {
                    g.assign(&filtered_result);
                }

                if let Ok(mut g) = wimg.try_lock() {
                    g.assign(&waterfall_buf);
                }

                //let mut waterfall_img=RgbImage::new(args.nch as u32, args.ntime as u32);
                //let mut waterfall_img=RgbImage::from_vec(args.nch as u32, args.ntime as u32, x).unwrap();

                //let mut g=wimg.lock().unwrap();
                /*
                                waterfall_buf.indexed_iter().for_each(|((i, j), &v)|{
                                    let v=v as f64;
                                    let c=colormap.get_color_normalized(v, min_value, max_value);
                                    waterfall_img.put_pixel(j as u32, i as u32, Rgb([c.0, c.1, c.2]));
                                });
                */

                //waterfall_img.save("./a.png");

                //*wimg.lock().unwrap()=x;
            }
            if tx_repaint.is_empty() {
                tx_repaint.send(()).unwrap();
            }
        }
    });

    let _th_repaint = std::thread::spawn(move || loop {
        rx_repaint.recv().unwrap();
        let ctx2 = ctx1.lock().unwrap();
        if let Some(ref x) = *ctx2 {
            x.request_repaint();
        }
    });

    let _th_daq = std::thread::spawn(move || {
        loop {
            //let mut buf = vec![Complex::<Ftype>::default(); stream.mtu().unwrap()];
            let mut buf = Vec::with_capacity(stream.mtu().unwrap());
            buf.resize(stream.mtu().unwrap(), Complex::default());
            let len = stream
                .read(&mut [&mut buf], 1_000_000)
                .expect("read failed");
            buf.resize(len, Complex::default());
            if !tx_raw.is_full() {
                tx_raw.send(buf).unwrap();
            } else {
                eprintln!("WARNING: daq queue full, data losting");
            }

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
    });

    let ctx1 = Arc::clone(&ctx);
    let mut native_options = eframe::NativeOptions::default();
    native_options.initial_window_size = Some(Vec2::new(800.0, 600.0));

    let wimg = waterfall_img_buf.clone();
    let sbuf = spectrum_buf.clone();
    let fmin = args.f0 - sampling_rate / 2.0;
    let fmax = args.f0 + sampling_rate / 2.0;
    let plot_spec = PlotSpec {
        fmin,
        fmax,
        ntime: args.ntime,
        nch: args.nch,
    };
    eframe::run_native(
        "Simple Example",
        native_options,
        Box::new(move |cc| Box::new(Simple::new(cc, ctx1, wimg, sbuf, plot_spec))),
    )
    .unwrap();
    /*
    th_daq.join().unwrap();
    th_filter.join().unwrap();
    th_channelize.join().unwrap();
    th_update_display.join().unwrap();
    */
    //stream.deactivate(None).expect("failed to deactivate");
}

struct Simple {
    pub waterfall_img: Arc<Mutex<Array2<f32>>>,
    pub spectrum_buf: Arc<Mutex<Array1<f32>>>,
    pub plot_spec: PlotSpec,
}

impl Simple {
    fn new(
        cc: &eframe::CreationContext<'_>,
        ctx_holder: Arc<Mutex<Option<Context>>>,
        wimg: Arc<Mutex<Array2<f32>>>,
        sbuf: Arc<Mutex<Array1<f32>>>,
        plot_spec: PlotSpec,
    ) -> Self {
        // Disable feathering as it causes artifacts
        let context = &cc.egui_ctx;

        context.tessellation_options_mut(|tess_options| {
            tess_options.feathering = false;
        });

        // Also enable light mode
        context.set_visuals(Visuals::light());
        let mut ctx1 = ctx_holder.lock().unwrap();
        *ctx1 = Some(context.clone());
        Self {
            waterfall_img: wimg,
            spectrum_buf: sbuf,
            plot_spec,
        }
    }
}

impl eframe::App for Simple {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            //println!("{}", ".");
            let root_area = EguiBackend::new(ui).into_drawing_area();
            root_area.fill(&WHITE).unwrap();

            //root_area.fill(&WHITE)?;

            //let root_area = root_area.titled("Image Title", ("sans-serif", 60)).unwrap();

            //let (upper, lower) = root_area.split_evenly((2,1));
            let (upper, lower) = {
                let a = root_area.split_evenly((2, 1));
                (a[0].clone(), a[1].clone())
            };

            let (w, h) = upper.dim_in_pixel();

            let (min_value, max_value) =
                self.waterfall_img
                    .lock()
                    .unwrap()
                    .iter()
                    .fold((1e99, -1e99), |a, &v| {
                        let v = v as f64;
                        (if a.0 < v { a.0 } else { v }, if a.1 > v { a.1 } else { v })
                    });

            if min_value == max_value {
                return;
            }
            //println!("{} {}", min_value, max_value);

            //let min_value=-100.0;
            //let max_value=db(max_value);
            //println!("{} {}" , min_value, max_value);

            let colormap = ViridisRGB;
            let x = self
                .waterfall_img
                .lock()
                .unwrap()
                .iter()
                .flat_map(|&v| {
                    let v = v as f64;
                    let v = v.max(min_value);
                    let v = v.min(max_value);
                    let c = colormap.get_color_normalized(v, min_value, max_value);
                    [c.0, c.1, c.2]
                })
                .collect::<Vec<_>>();

            let waterfall = DynamicImage::ImageRgb8(
                RgbImage::from_vec(self.plot_spec.nch as u32, self.plot_spec.ntime as u32, x)
                    .unwrap(),
            )
            .resize_exact(w, h, Nearest);

            let bmp: BitMapElement<_> = (
                (self.plot_spec.fmin, -(self.plot_spec.ntime as f64)),
                waterfall,
            )
                .into();

            //let _x_axis = (-3.4f32..3.4).step(0.1);

            let mut cc = ChartBuilder::on(&upper)
                //.margin(1)
                //.set_label_area_size(LabelAreaPosition::Left, 5)
                //.set_label_area_size(LabelAreaPosition::Bottom, 5)
                .set_all_label_area_size(5)
                //.caption("Sine and Cosine", ("sans-serif", 40))
                .build_cartesian_2d(
                    (self.plot_spec.fmin / 1e6)..(self.plot_spec.fmax / 1e6),
                    0.0..(-(self.plot_spec.ntime as f64)),
                )
                .unwrap();

            cc.configure_mesh().draw().unwrap();
            cc.draw_series(std::iter::once(bmp)).unwrap();

            let spec = self.spectrum_buf.lock().unwrap();
            let (min_value, max_value) = spec
                .iter()
                .skip(self.plot_spec.nch / 4)
                .take(self.plot_spec.nch / 2)
                .fold((1e99, -1e99), |a, &v| {
                    let v = v as f64;
                    (if a.0 < v { a.0 } else { v }, if a.1 > v { a.1 } else { v })
                });

            let mut cc = ChartBuilder::on(&lower)
                //.set_label_area_size(LabelAreaPosition::Left, 1)
                //.set_label_area_size(LabelAreaPosition::Bottom, 1)
                .set_all_label_area_size(5)
                .build_cartesian_2d(
                    (self.plot_spec.fmin / 1e6)..(self.plot_spec.fmax / 1e6),
                    db(min_value) - 1_f64..db(max_value) + 1_f64,
                )
                .unwrap();

            //println!("{} {}", min_value, max_value);

            cc.configure_mesh().draw().unwrap();
            cc.draw_series(LineSeries::new(
                (0..self.plot_spec.nch).map(|ich| {
                    (
                        (ich as f64 / self.plot_spec.nch as f64
                            * (self.plot_spec.fmax - self.plot_spec.fmin)
                            + self.plot_spec.fmin)
                            / 1e6,
                        db(spec[ich] as f64),
                    )
                }),
                &BLUE,
            ))
            .unwrap();

            root_area.present().unwrap();
        });
    }
}
