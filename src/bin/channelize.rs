use clap::Parser;

use image::{imageops::FilterType::Nearest, DynamicImage, RgbImage};
use ndarray::{s, Array1, Array2};

//use rayon::prelude::*;

use num::complex::Complex;
use soapy_spec_acc::daq::run_daq;
use soapysdr::{Device, Direction};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, CentralPanel, Context, Key, Slider, TopBottomPanel, Vec2, Visuals};
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

#[derive(Clone)]
struct State {
    freq: f64, 
    samp_rate: f64,
    min_ch: usize,
    max_ch: usize,
    yscale_min: f64,
    yscale_max: f64,
    ntime: usize,
    nch: usize,
    device: Device,
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
    let sdr_stream = device.rx_stream::<Complex<Ftype>>(&[0]).unwrap();

    let ctx = Arc::new(Mutex::new(Option::<Context>::default()));
    let ctx1 = Arc::clone(&ctx);

    //let waterfall_img_buf = Arc::new(Mutex::new(vec![0_u8; (args.ntime * args.nch * 3)]));
    let waterfall_img_buf = Arc::new(Mutex::new(Array2::<f32>::zeros((args.ntime, args.nch))));
    let spectrum_buf = Arc::new(Mutex::new(Array1::<f32>::zeros(args.nch)));

    let wimg = waterfall_img_buf.clone();
    let sbuf = spectrum_buf.clone();

    let (tx_repaint, rx_repaint) = bounded(1);

    let rx_averaged = run_daq(sdr_stream, args.nch, args.ntap, args.n_average);
    device.set_frequency(Direction::Rx, 0, args.f0, ()).unwrap();

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

    let ctx1 = Arc::clone(&ctx);
    let mut native_options = eframe::NativeOptions::default();
    native_options.initial_window_size = Some(Vec2::new(900.0, 600.0));

    let wimg = waterfall_img_buf.clone();
    let sbuf = spectrum_buf.clone();
    let fmin = args.f0 - sampling_rate / 2.0;
    let fmax = args.f0 + sampling_rate / 2.0;
    let state = State {
        freq: args.f0,
        samp_rate: sampling_rate,
        min_ch: 0,
        max_ch: args.nch - 1,
        yscale_max: 1.0,
        yscale_min: 0.0,
        ntime: args.ntime,
        nch: args.nch,
        device,
    };
    eframe::run_native(
        "PlotWindow Example",
        native_options,
        Box::new(move |cc| Box::new(PlotWindow::new(cc, ctx1, wimg, sbuf, state))),
    )
    .unwrap();
    /*
    th_daq.join().unwrap();
    th_filter.join().unwrap();
    th_channelize.join().unwrap();
    th_update_display.join().unwrap();
    */
    //sdr_stream.deactivate(None).expect("failed to deactivate");
}

struct PlotWindow {
    pub waterfall_img: Arc<Mutex<Array2<f32>>>,
    pub spectrum_buf: Arc<Mutex<Array1<f32>>>,
    pub state: State,
}

impl PlotWindow {
    fn new(
        cc: &eframe::CreationContext<'_>,
        ctx_holder: Arc<Mutex<Option<Context>>>,
        wimg: Arc<Mutex<Array2<f32>>>,
        sbuf: Arc<Mutex<Array1<f32>>>,
        state: State,
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
            state,
        }
    }
}

impl eframe::App for PlotWindow {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (min_value, max_value) =
            self.waterfall_img
                .lock()
                .unwrap().slice(s![..,self.state.min_ch..=self.state.max_ch])
                .iter()
                .fold((1e99, -1e99), |a, &v| {
                    let v = v as f64;
                    (if a.0 < v { a.0 } else { v }, if a.1 > v { a.1 } else { v })
                });

        if min_value == max_value {
            return;
        }

        TopBottomPanel::bottom("playmenu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("min ch");

                let mut min_ch = self.state.min_ch;
                let mut max_ch = self.state.max_ch;
                if ui
                    .add(Slider::new(&mut min_ch, 0..=(self.state.nch - 1)))
                    .changed()
                {
                    self.state.min_ch = min_ch;
                    if self.state.max_ch < self.state.min_ch + 1 {
                        self.state.max_ch = min_ch + 1;
                    }
                }

                ui.label("max ch");
                if ui
                    .add(Slider::new(&mut max_ch, 0..=(self.state.nch - 1)))
                    .changed()
                {
                    self.state.max_ch = max_ch;
                    if self.state.max_ch < self.state.min_ch + 1 {
                        self.state.min_ch = max_ch - 1;
                    }
                }

                ui.label("zoom in");
                let mut yscale_max = self.state.yscale_max;
                let mut yscale_min = self.state.yscale_min;

                if ui.add(Slider::new(&mut yscale_min, 0.0..=1.0)).changed() {
                    self.state.yscale_min = yscale_min;
                    if self.state.yscale_max - self.state.yscale_min < 0.01 {
                        self.state.yscale_max = self.state.yscale_min + 0.01;
                    }
                }

                if ui.add(Slider::new(&mut yscale_max, 0.0..=1.0)).changed() {
                    self.state.yscale_max = yscale_max;
                    //self.state.yscale_min=self.state.yscale_min.max(yscale_max);
                    if self.state.yscale_max - self.state.yscale_min < 0.01 {
                        self.state.yscale_min = self.state.yscale_max - 0.01;
                    }
                }

                ui.label(format!("F={} MHz", self.state.freq/1e6));
            })
        });

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

            let df = self.state.samp_rate / self.state.nch as f64;
            let fmin_raw=self.state.freq-self.state.samp_rate/2.0;
            let fmax_raw=self.state.freq+self.state.samp_rate/2.0;
            let fmin_display = self.state.min_ch as f64 * df + fmin_raw;
            let fmax_display = self.state.max_ch as f64 * df + fmin_raw;
            let x1 = ((fmin_display - fmin_raw)
                / self.state.samp_rate
                * self.state.nch as f64) as u32;
            let x2 = ((fmax_display - fmin_raw)
                / self.state.samp_rate
                * self.state.nch as f64) as u32;

            let waterfall = DynamicImage::ImageRgb8(
                RgbImage::from_vec(self.state.nch as u32, self.state.ntime as u32, x).unwrap(),
            )
            .crop(x1, 0, x2 - x1, self.state.ntime as u32)
            .resize_exact(w - 15, h - 25, Nearest);

            let bmp: BitMapElement<_> =
                ((fmin_display, -(self.state.ntime as f64)), waterfall).into();

            //let _x_axis = (-3.4f32..3.4).step(0.1);

            let mut cc = ChartBuilder::on(&upper)
                //.margin(1)
                .set_label_area_size(LabelAreaPosition::Top, 25)
                .set_label_area_size(LabelAreaPosition::Left, 5)
                .set_label_area_size(LabelAreaPosition::Right, 5)
                //.set_all_label_area_size(5)
                //.caption("Sine and Cosine", ("sans-serif", 40))
                .build_cartesian_2d(
                    (fmin_raw / 1e6)..(fmax_raw / 1e6),
                    0.0..(-(self.state.ntime as f64)),
                )
                .unwrap();

            cc.configure_mesh().draw().unwrap();
            cc.draw_series(std::iter::once(bmp)).unwrap();

            let spec = self.spectrum_buf.lock().unwrap();
            let (min_value, max_value) = spec
                .iter()
                .enumerate()
                .filter(|&(ich, _)| {
                    ich + 10 >= self.state.min_ch && ich <= self.state.max_ch + 10
                    //true
                })
                //.skip(self.state.nch / 4)
                //.take(self.state.nch / 2)
                .fold((1e99, -1e99), |a, (_, &v)| {
                    let v = v as f64;
                    (if a.0 < v { a.0 } else { v }, if a.1 > v { a.1 } else { v })
                });
            //println!("{} {}", min_value, max_value);
            let y1 = db(min_value) - 1_f64;
            let y2 = db(max_value) + 1_f64;
            //println!("{} {}", min_value, max_value);
            let ys1 = (y2 - y1) * self.state.yscale_min + y1;
            let ys2 = (y2 - y1) * self.state.yscale_max + y1;

            let mut cc = ChartBuilder::on(&lower)
                .set_label_area_size(LabelAreaPosition::Left, 5)
                .set_label_area_size(LabelAreaPosition::Right, 5)
                .set_label_area_size(LabelAreaPosition::Bottom, 25)
                //.set_all_label_area_size(5)
                .build_cartesian_2d(
                    (fmin_display / 1e6 - 0.1)..(fmax_display / 1e6 + 0.1),
                    ys1..ys2,
                )
                .unwrap();

            //println!("{} {}", min_value, max_value);

            cc.configure_mesh().draw().unwrap();
            cc.draw_series(LineSeries::new(
                (0..self.state.nch).map(|ich| {
                    (
                        (ich as f64 / self.state.nch as f64
                            * self.state.samp_rate
                            + fmin_raw)
                            / 1e6,
                        db(spec[ich] as f64),
                    )
                }),
                &BLUE,
            ))
            .unwrap();

            root_area.present().unwrap();
            let df = if ctx.input(|input| input.key_pressed(Key::D)) {
                0.1e6
            } else if ctx.input(|input| input.key_pressed(Key::S)) {
                1e6
            } else if ctx.input(|input| input.key_pressed(Key::A)) {
                5e6
            } else if ctx.input(|input| input.key_pressed(Key::C)) {
                -0.1e6
            } else if ctx.input(|input| input.key_pressed(Key::X)) {
                -1e6
            } else if ctx.input(|input| input.key_pressed(Key::Z)) {
                -5e6
            } else {
                0.0
            };

            if df != 0.0_f64 {
                let f = self.state.device.frequency(Direction::Rx, 0).unwrap();
                self.state
                    .device
                    .set_frequency(Direction::Rx, 0, f + df, ())
                    .unwrap();
                let f = f + df;
                self.state.freq=f;
                println!("freq changed to {}", f);
            }
        });
    }
}
