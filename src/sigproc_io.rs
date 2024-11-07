use binrw::{binrw, BinRead, BinWrite};
//use crate::pulsar::Pulsar;
use num::traits::{Float, NumCast};

use core::panic;
use std::io::{Read, Write};



#[binrw]
#[brw(little)]
#[derive(Clone)]
pub struct SpStr {
    pub len: u32,
    #[br(little, count = len)]
    #[bw(little)]
    pub content: Vec<u8>,
}

impl std::fmt::Debug for SpStr {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        String::from_utf8(self.content.clone())
            .unwrap()
            .fmt(formatter)
    }
}

impl SpStr {
    pub fn new(c: &str) -> SpStr {
        let content: Vec<u8> = c.as_bytes().iter().cloned().collect();
        SpStr {
            len: content.len() as u32,
            content,
        }
    }
}

#[derive(Clone)]
#[binrw]
#[brw(little)]

pub struct KvPair<T>
where
    for<'a> T: Clone + BinRead<Args<'a> = ()> + BinWrite<Args<'a> = ()>,
    //<T as BinWrite>::Args<'_>=()
{
    pub key: SpStr,
    pub value: T,
}

impl<T> std::fmt::Debug for KvPair<T>
where
    for<'a> T: std::fmt::Debug + Clone + BinRead<Args<'a> = ()> + BinWrite<Args<'a> = ()>,
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.key.fmt(fmt)?;
        write!(fmt, " : ")?;
        self.value.fmt(fmt)
    }
}

impl<T> KvPair<T>
where
    for<'a> T: std::fmt::Debug + Clone + BinRead<Args<'a> = ()> + BinWrite<Args<'a> = ()>,
{
    pub fn new(k: &str, v: T) -> KvPair<T> {
        KvPair {
            key: SpStr::new(k),
            value: v,
        }
    }
}

#[derive(Clone)]

pub enum HeaderItem {
    HeaderStart,
    HeaderEnd,
    StringItem(KvPair<SpStr>),
    IntItem(KvPair<u32>),
    DoubleItem(KvPair<f64>),
}

impl std::fmt::Debug for HeaderItem {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        use HeaderItem::*;
        match self {
            HeaderStart => "HeaderStart".fmt(fmt),
            HeaderEnd => "HeaderEnd".fmt(fmt),
            StringItem(ref x) => x.fmt(fmt),
            IntItem(ref x) => x.fmt(fmt),
            DoubleItem(ref x) => x.fmt(fmt),
        }
    }
}

impl BinRead for HeaderItem {
    type Args<'a> = ();

    fn read_options<R: Read + std::io::prelude::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        let k = SpStr::read_options(reader, endian, args)?;
        let item = match String::from_utf8(k.content.clone()).unwrap().as_str() {
            "telescope_id" | "machine_id" | "data_type" | "barycentric" | "pulsarcentric"
            | "nbits" | "nsamples" | "nchans" | "nifs" => {
                let v = u32::read_options(reader, endian, args)?;
                HeaderItem::IntItem(KvPair { key: k, value: v })
            }
            "rawdatafile" | "source_name" => {
                let v = SpStr::read_options(reader, endian, args)?;
                HeaderItem::StringItem(KvPair { key: k, value: v })
            }
            "az_start" | "za_start" | "src_raj" | "src_dej" | "tstart" | "tsamp" | "fch1"
            | "foff" | "fchannel" | "refdm" | "period" => {
                let v = f64::read_options(reader, endian, args)?;
                HeaderItem::DoubleItem(KvPair { key: k, value: v })
            }
            "HEADER_START" => HeaderItem::HeaderStart,
            "HEADER_END" => HeaderItem::HeaderEnd,
            _ => panic!(),
        };
        Ok(item)
    }
}

impl BinWrite for HeaderItem {
    type Args<'a> = ();

    fn write_options<W: Write + std::io::prelude::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<()> {
        match self {
            HeaderItem::HeaderStart => SpStr::new("HEADER_START").write_options(writer, endian, args),
            HeaderItem::HeaderEnd => SpStr::new("HEADER_END").write_options(writer, endian, args),
            HeaderItem::IntItem(ref x) => x.write_options(writer, endian, args),
            HeaderItem::StringItem(ref x) => x.write_options(writer, endian, args),
            HeaderItem::DoubleItem(ref x) => x.write_options(writer, endian, args),
        }
    }
}

#[derive(Clone)]
pub struct Header {
    start: HeaderItem,
    pub items: Vec<HeaderItem>,
    end: HeaderItem,
}

impl BinRead for Header {
    type Args<'a> = ();

    fn read_options<R: Read + std::io::prelude::Seek>(
            reader: &mut R,
            endian: binrw::Endian,
            args: Self::Args<'_>,
        ) -> binrw::prelude::BinResult<Self> {
        match HeaderItem::read_options(reader, endian, args)?{
            HeaderItem::HeaderStart=>(),
            _=>panic!()
        }

        let mut items=vec![];
        loop{
            let item=HeaderItem::read_options(reader, endian, args)?;
            if let HeaderItem::HeaderEnd=item{
                break;
            }else{
                items.push(item);
            }
        }

        Ok(Self{
            start:HeaderItem::HeaderStart,
            items,
            end:HeaderItem::HeaderEnd
        })
    }
}


impl BinWrite for Header{
    type Args<'a> = ();

    fn write_options<W: Write + std::io::prelude::Seek>(
            &self,
            writer: &mut W,
            endian: binrw::Endian,
            args: Self::Args<'_>,
        ) -> binrw::prelude::BinResult<()> {
        HeaderItem::HeaderStart.write_options(writer, endian, args)?;
        for x in &self.items{
            x.write_options(writer, endian, args)?;
        }
        HeaderItem::HeaderEnd.write_options(writer, endian, args)
    }
}

impl std::fmt::Debug for Header {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.start.fmt(fmt)?;
        writeln!(fmt)?;
        for i in &self.items {
            i.fmt(fmt)?;
            writeln!(fmt)?;
        }
        self.end.fmt(fmt)
    }
}

impl std::default::Default for Header {
    fn default() -> Header {
        Header {
            start: HeaderItem::HeaderStart,
            items: vec![],
            end: HeaderItem::HeaderEnd,
        }
    }
}

impl Header {
    /*
    pub fn new()->Header{
        Header{start:HeaderItem::HeaderStart, items:vec![], end:HeaderItem::HeaderEnd}
    }*/

    pub fn new<T>(fch1: T, nch: usize, foff: T, t0: f64, tsamp: f64) -> Header
    where
        T: Float + std::fmt::Debug,
    {
        //let two = T::one() + T::one();
        //let fs_mhz = foff * T::from(nch).unwrap() * two;
        //let dt = T::one() / (fs_mhz * T::from(1e6).unwrap());
        //eprintln!("dt= {:?}", dt);
        let mut header = Self::default();
        header.push_item(HeaderItem::StringItem(KvPair::new(
            "source_name",
            SpStr::new("fake"),
        )));
        header.push_item(HeaderItem::IntItem(KvPair::new("machine_id", 0)));
        header.push_item(HeaderItem::IntItem(KvPair::new("telescope_id", 0)));
        header.push_item(HeaderItem::IntItem(KvPair::new("data_type", 1)));
        header.push_item(HeaderItem::DoubleItem(KvPair::new("fch1", <f64 as NumCast>::from(fch1).unwrap())));
        header.push_item(HeaderItem::DoubleItem(KvPair::new("foff", <f64 as NumCast>::from(foff).unwrap())));
        header.push_item(HeaderItem::IntItem(KvPair::new("nchans", nch as u32)));
        header.push_item(HeaderItem::IntItem(KvPair::new("barycentric", 1 as u32)));
        header.push_item(HeaderItem::IntItem(KvPair::new("nbits", 32)));
        header.push_item(HeaderItem::DoubleItem(KvPair::new("tstart", t0)));
        header.push_item(HeaderItem::DoubleItem(KvPair::new("tsamp", tsamp)));
        header.push_item(HeaderItem::DoubleItem(KvPair::new("src_raj", 0.0)));
        header.push_item(HeaderItem::DoubleItem(KvPair::new("src_dej", 900000.0)));
        header.push_item(HeaderItem::IntItem(KvPair::new("nifs", 1)));
        header
    }

    pub fn push_item(&mut self, item: HeaderItem) {
        self.items.push(item)
    }

    pub fn nifs(&self) -> usize {
        for i in &self.items {
            if let HeaderItem::IntItem(x) = i {
                if String::from_utf8(x.key.content.clone()).unwrap() == "nifs" {
                    return x.value as usize;
                }
            }
        }
        1
    }

    pub fn nchans(&self) -> usize {
        for i in &self.items {
            if let HeaderItem::IntItem(x) = i {
                if String::from_utf8(x.key.content.clone()).unwrap() == "nchans" {
                    return x.value as usize;
                }
            }
        }
        unreachable!()
    }

    pub fn tsamp(&self) -> f64 {
        for i in &self.items {
            if let HeaderItem::DoubleItem(x) = i {
                if String::from_utf8(x.key.content.clone()).unwrap() == "tsamp" {
                    return x.value as f64;
                }
            }
        }
        unreachable!()
    }

    pub fn set_tsamp(&mut self, tsamp: f64) {
        for i in &mut self.items {
            if let HeaderItem::DoubleItem(x) = i {
                if String::from_utf8(x.key.content.clone()).unwrap() == "tsamp" {
                    x.value = tsamp;
                    return;
                }
            }
        }
        unreachable!()
    }

    pub fn nbits(&self) -> usize {
        for i in &self.items {
            if let HeaderItem::IntItem(x) = i {
                if String::from_utf8(x.key.content.clone()).unwrap() == "nbits" {
                    return x.value as usize;
                }
            }
        }
        unreachable!()
    }
}
