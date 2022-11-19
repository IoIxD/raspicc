#![allow(dead_code)]

use libc::types::common::c95::c_void;

use videocore::dispmanx::{ElementHandle, ResourceHandle, VCAlpha, Clamp};
use videocore::{dispmanx,image,bcm_host};
use core::time;
use std::sync::mpsc::{Sender, Receiver, self};
use std::{thread, option};
use time::Duration;
use image::ImageType::_8BPP;
use image::Rect;

type InitFunc = fn(image: &Vec<u8>);
type DrawFunc = fn(image: &Vec<u8>, next_resource: usize);

struct RenderShared {
    display: dispmanx::DisplayHandle,
    element: dispmanx::ElementHandle,
    resource: [dispmanx::ResourceHandle; 2],

    image: Vec<u8>,
    width: u32,
    image_rect: *const image::Rect,
    draw_func: DrawFunc,
    delay: Duration,

    channel: (Sender<u32>, Receiver<u32>),

    done: bool,
}

macro_rules! rust_type_to_void {
    ($a:expr) => {
        &mut $a as *mut _ as *mut c_void
    };
}

struct RenderSharedOptions {
    device_id: u32,
    width: u32,
    height: u32,
    offset: u32, 
    fixed: u32,
    init_func: InitFunc, 
    draw_func: DrawFunc, 
    delay: i32, 
    level: u32
}

impl RenderShared {
    fn new(options: RenderSharedOptions) -> RenderShared {
        let mut ret: bool;
        let display = dispmanx::display_open(options.device_id);
        let mut resource: [ResourceHandle; 2] = [0; 2];

        let level = (options.level * 31) / 100;
        let white = 0x0020 | level | level << 6 | level << 11;

        let mut image: Vec<u32> = vec![0; (pitch(options.width) * options.height as i32) as usize];
        let mut palette: Vec<u32> = vec!(0x0, white, 0xf000);
    
        let delay = {
            if options.delay > -1 {
                Duration::from_millis(options.delay as u64)
            } else {
                Duration::from_millis(2000)
            }
        };

        let image_rect = Rect {
            x: (options.offset+options.fixed) as i32,
            y: 0,
            width: (options.width - (options.offset + options.fixed)) as i32,
            height: (options.height) as i32,
        };

        for n in 0..2 {
            resource[n] = dispmanx::resource_create(_8BPP, options.width, options.height, *&image.as_mut_ptr());
            assert!(resource[n] == 1);
            ret = dispmanx::resource_set_palette(resource[n], rust_type_to_void!(palette), 0, palette.len() as i32);
            assert!(!ret);
            ret = dispmanx::resource_write_data(resource[n], _8BPP, pitch(options.width) as i32, rust_type_to_void!(image), &image_rect as *const Rect);
            assert!(!ret);
        }

        let update: dispmanx::UpdateHandle = dispmanx::update_start(10);
        assert!(update == 1);

        let (mut src_rect, mut dst_rect): (image::Rect, image::Rect);
        src_rect = image::Rect{
            x: 0,
            y: 0,
            width: (options.width << 16) as i32,
            height: (options.height << 16) as i32,
        };
        dst_rect = image::Rect{
            x: 0,
            y: 0,
            width: 720,
            height: (options.height << 16) as i32,
        };
    
        let mut alpha: VCAlpha = VCAlpha {
            flags: dispmanx::FlagsAlpha::FROM_SOURCE,
            opacity: 255,
            mask: 0,
        };

        let mut clamp: Clamp = Clamp {
            mode: dispmanx::FlagsClamp::NONE,
            key_mask: dispmanx::FlagsKeymask::OVERRIDE,
            key_value: rust_type_to_void!(0),
            replace_value: 0,
        };
        
        let element: ElementHandle = dispmanx::element_add(
            update, 
            display, 
            2000, 
            &mut dst_rect as *mut Rect, 
            resource[2], 
            &mut src_rect as *mut Rect, 
            dispmanx::DISPMANX_PROTECTION_NONE, 
            &mut alpha as *mut VCAlpha, 
            &mut clamp as *mut Clamp, 
            dispmanx::Transform::NO_ROTATE
        );

        RenderShared {
            display,
            element,
            resource,
            image: Vec::new(),
            width: options.width,
            image_rect: &image_rect as *const Rect,
            draw_func: options.draw_func,
            delay: delay,
            channel: mpsc::channel(),
            done: false,
        }
    }

    fn start(&self) {
        thread::spawn(|| {
            let none_arg = rust_type_to_void!(IHateC{});

            let mut ret: bool = false;
            let mut next_resource: usize = 0;
            let mut update: dispmanx::UpdateHandle;
            
            while !&self.done {
                update = dispmanx::update_start(10);
                let resource = match self.resource.get(next_resource) {
                    Some(a) => *a,
                    None => 0,
                };
                ret = dispmanx::element_change_source(update, (&self).element, resource);
                assert!(!ret);
                _ = &self.channel.1.recv();
        
                // RPi firmware sends the vsync callback with just a few ms to spare before it
                // paints the next frame, so we must put a delay here to guarantee that we
                // always miss the current frame, in order to get a stable framerate.
                // See https://github.com/raspberrypi/firmware/issues/1182
                // and https://github.com/raspberrypi/firmware/issues/1154
                thread::sleep((&self).delay);
                ret = dispmanx::update_submit(update, deeznuts, none_arg);
                assert!(!ret);
                next_resource ^= 1;
                (&self.draw_func)(&self.image, next_resource);
                ret = dispmanx::resource_write_data(resource, _8BPP, pitch((&self).width), rust_type_to_void!(&self.image), (&self).image_rect);
                assert!(!ret);
            }
        
            _ = ret;
        });


    }
}

extern "C" fn deeznuts(_: dispmanx::UpdateHandle, _: *mut c_void) {
        
}

extern "C" fn vsync_callback(_: dispmanx::UpdateHandle, anon_render_shared: *mut c_void) {
    unsafe {
        let r: *mut RenderShared = anon_render_shared as *mut RenderShared;
        _ = (*r).channel.0.send(0);
    }
}

struct IHateC;

fn render_thread(r: &RenderShared) {

}


fn pitch(val: u32) -> i32 {
    align_up(val,32)
}
fn align_up(x: u32, y: u32) -> i32 {
    ((x + (y)-1) & !((y)-1)) as i32
}

fn render_start(width: u32, height: u32, offset: u32, fixed: u32, init_func: InitFunc, draw_func: DrawFunc, delay: i32, level: u32) {
    let r: RenderShared = RenderShared::new(RenderSharedOptions{
        device_id: 0,
        width,
        height,
        offset,
        fixed,
        init_func,
        draw_func,
        delay,
        level,
    });
}