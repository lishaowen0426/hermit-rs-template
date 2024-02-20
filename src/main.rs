//use tract_onnx::prelude::*;
use std::fs::{create_dir, read_dir, File, metadata};
use std::include_bytes;
use std::io::{Read, Seek, SeekFrom, Write};

//static MODEL: &'static [u8;14246826] = include_bytes!("/home/sw/hermit/mobilenetv2-7.onnx");
fn main() {
    let d = read_dir("/myfs").unwrap();
    for ent in d{
        if let Ok(entry) = ent{
            if let Ok(ft) = entry.file_type(){
println!("{:?}, dir:{:?}, file:{:?}", entry.path(), ft.is_dir(), ft.is_file());
            }
            println!("meta:{:?}", entry.metadata());
        }
    }

    return;
}
