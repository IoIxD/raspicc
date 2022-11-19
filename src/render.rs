#![allow(dead_code)]

use libc::types::common::c95::c_void;

use videocore::{dispmanx,image,bcm_host};
use core::time;
use std::sync::mpsc::{Sender, Receiver, self};
use std::thread;
use time::Duration;
use image::ImageType::_8BPP;

type InitFunc = fn(image: &Vec<u8>);
type DrawFunc = fn(image: &Vec<u8>, next_resource: usize);

struct RenderShared<'a> {
    display: dispmanx::DisplayHandle,
    element: dispmanx::ElementHandle,
    resource: [dispmanx::ResourceHandle; 2],

    image: &'a Vec<u8>,
    width: u32,
    image_rect: *const image::Rect,
    draw_func: DrawFunc,
    delay: Duration,

    channel: (Sender<u32>, Receiver<u32>),

    done: bool,
}

extern "C" fn deeznuts(_: dispmanx::UpdateHandle, _: *mut c_void) {
        
}

extern "C" fn vsync_callback(_: dispmanx::UpdateHandle, anon_render_shared: *mut c_void) {
    unsafe {
        let r: *mut RenderShared = anon_render_shared as *mut RenderShared;
        _ = (*r).channel.0.send(0);
    }
}

macro_rules! rust_type_to_void {
    ($a:expr) => {
        &mut $a as *mut _ as *mut c_void
    };
}

struct IHateC;

fn render_thread(r: &RenderShared) {
    let none_arg = rust_type_to_void!(IHateC{});

    let mut ret: bool = false;
    let mut next_resource: usize = 0;
    let mut update: dispmanx::UpdateHandle;
    
    while !r.done {
        update = dispmanx::update_start(10);
        ret = dispmanx::element_change_source(update, r.element, *r.resource.get(next_resource).unwrap());
        assert!(!ret);
        _ = r.channel.1.recv();

        // RPi firmware sends the vsync callback with just a few ms to spare before it
        // paints the next frame, so we must put a delay here to guarantee that we
        // always miss the current frame, in order to get a stable framerate.
        // See https://github.com/raspberrypi/firmware/issues/1182
        // and https://github.com/raspberrypi/firmware/issues/1154
        thread::sleep(r.delay);
        ret = dispmanx::update_submit(update, deeznuts, none_arg);
        assert!(!ret);
        next_resource ^= 1;
        (r.draw_func)(r.image, next_resource);
        ret = dispmanx::resource_write_data(*r.resource.get(next_resource).unwrap(), _8BPP, pitch(r.width), rust_type_to_void!(&r.image), r.image_rect);
        assert!(!ret);
    }

    _ = ret;

}


fn pitch(val: u32) -> i32 {
    align_up(val,32)
}
fn align_up(x: u32, y: u32) -> i32 {
    ((x + (y)-1) & !((y)-1)) as i32
}

fn render_start(width: u32, height: u32, offset: u32, fixed: u32, init_func: InitFunc, draw_func: DrawFunc, delay: u32, level: u32) {
    let mut ret: bool;

    let level = (level * 31) / 100;
    let white = 0x0020 | level | level << 6 | level << 11;

    let (src_rect, dst_rect): (image::Rect, image::Rect);
    
    let update: dispmanx::UpdateHandle;
    let mut image: Vec<u32> = Vec::new();
    let mut palette: Vec<u32> = vec!(0x0, white, 0xf000);

    let r: RenderShared;
    r.channel = mpsc::channel();

    let display = dispmanx::display_open(0);
    let resource: [dispmanx::ResourceHandle; 2];
    for n in 0..1 {
        resource[n] = dispmanx::resource_create(_8BPP, width, height, *&image.as_mut_ptr());
        assert!(resource[n] == 1);
        ret = dispmanx::resource_set_palette(resource[n], rust_type_to_void!(palette), 0, palette.len() as i32);
        assert!(!ret);
        ret = dispmanx::resource_write_data(resource[n], _8BPP, pitch(width) as i32, rust_type_to_void!(r.image), r.image_rect);
        assert!(!ret);
    }



}