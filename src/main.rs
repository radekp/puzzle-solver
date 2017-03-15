extern crate sdl2;
extern crate image;

use std::cmp;

use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::image::LoadTexture;
use sdl2::render::TextureQuery;
use sdl2::render::Renderer;
use sdl2::render::Texture;

// SDL window size - puzzle pieces bitmap must fit even with rotation
const WND_WIDTH: usize = 1024;
const WND_HEIGHT: usize = 1024;

const COL_MASK_MATERIAL: u8 = 1 << 4;
const COL_MASK_BORDER: u8 = 1 << 7;
const COL_MASK_JAG: u8 = 1 << 5;

struct URect {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

// Near point iterator
// Iterates points in spiral centered at cx,cy
//
//     9 ....
//       1 2 3
//       8   4
//       7 6 5
//
// If a==0 it will start in cx,cy orherwise a is square side on start
fn near_iter_begin(cx: i32, cy: i32, start_a: i32) -> (i32, i32, i32) {
    return (cx - start_a, cy - start_a, start_a);
}

// Return next point in spiral
fn near_iter_next(cx: i32, cy: i32, prev_x: i32, prev_y: i32, prev_a: i32) -> (i32, i32, i32) {

    let mut x = prev_x;
    let mut y = prev_y;
    let mut a = prev_a;

    if x == cx && y == cy {
        return (cx - 1, cy - 1, a);
    }

    if y == cy - a {
        x += 1;
        if x - cx <= a {
            return (x, y, a);
        }
        x = cx + a;
        y = cy - a + 1;
        return (x, y, a);
    }

    if x == cx + a {
        y += 1;
        if y - cy <= a {
            return (x, y, a);
        }
        x = cx + a - 1;
        y = cy + a;
        return (x, y, a);
    }

    if y == cy + a {
        x -= 1;
        if cx - x <= a {
            return (x, y, a);
        }
        x = cx - a;
        y = cy + a - 1;
        return (x, y, a);
    }

    y -= 1;
    if y > cy - a {
        return (x, y, a);
    }
    a += 1;
    return (cx - a, cy - a, a);
}

// Detect piece color - in my case they are dark blue
fn detect_material(pixels: &mut Vec<u8>, x: usize, y: usize) -> bool {
    let offset = 3 * (WND_WIDTH * y + x);
    let r = pixels[offset] as i32;
    let b = pixels[offset + 2] as i32;
    if b - r > 30 {
        pixels[offset] = COL_MASK_MATERIAL;
        pixels[offset + 1] = COL_MASK_MATERIAL;
        pixels[offset + 2] = COL_MASK_MATERIAL;
        return true;
    }
    pixels[offset] = 0;
    pixels[offset + 1] = 0;
    pixels[offset + 2] = 0;
    return false;
}

// Draw border pixels with red=127
fn detect_border(pixels: &mut Vec<u8>, x: usize, y: usize) {
    let offset = 3 * (WND_WIDTH * y + x);
    if pixels[offset] == 0 {
        return;
    }
    if x > 0 {
        let offset_xm = 3 * (WND_WIDTH * y + x - 1);
        if pixels[offset_xm] & COL_MASK_MATERIAL == 0 {
            pixels[offset] |= COL_MASK_BORDER;
        }
    }
    if y > 0 {
        let offset_ym = 3 * (WND_WIDTH * (y - 1) + x);
        if pixels[offset_ym] & COL_MASK_MATERIAL == 0 {
            pixels[offset] |= COL_MASK_BORDER;
        }
    }
    let offset_xp = 3 * (WND_WIDTH * y + x + 1);
    if pixels[offset_xp] & COL_MASK_MATERIAL == 0 {
        pixels[offset] |= COL_MASK_BORDER;
    }
    let offset_yp = 3 * (WND_WIDTH * (y + 1) + x);
    if pixels[offset_yp] & COL_MASK_MATERIAL == 0 {
        pixels[offset] |= COL_MASK_BORDER;
    }
}

// Remove jags from puzzle:
//          __
//         /  \			< removes this line
//        |    |		< and this
//        \   /         < and this, because they are thinner then width_limit
//   ------   ------    < keeps this line
//  /               \   < and this line, because they are above width_limit
//
fn detect_jags(pixels: &mut Vec<u8>,
               max: usize,
               plus_min_dst: usize,
               width_limit: usize,
               height_limit: usize) {

    // Foreach row
    for y in plus_min_dst..max {
        // Compute left and right coordinate
        let mut left = usize::max_value();
        let mut right = usize::max_value();
        for x in 0..max {
            let offset_up = 3 * (WND_WIDTH * (y - plus_min_dst) + x);
            if pixels[offset_up] & COL_MASK_BORDER == 0 {
                let offset_down = 3 * (WND_WIDTH * (y + plus_min_dst) + x);
                if pixels[offset_down] & COL_MASK_BORDER == 0 {
                    continue;
                }
            }
            if left == usize::max_value() {
                left = x;
            }
            right = x;
        }
        // Is the shape wide enough?
        if right - left >= width_limit {
            continue;
        }
        for x in 0..max {
            let offset = 3 * (WND_WIDTH * y + x);
            if pixels[offset] & COL_MASK_MATERIAL != 0 {
                pixels[offset] |= COL_MASK_JAG;
            }
        }
    }

    // Same for columns
    for x in plus_min_dst..max {
        let mut top = usize::max_value();
        let mut bottom = usize::max_value();
        for y in 0..max {
            let offset_left = 3 * (WND_WIDTH * y + x - plus_min_dst);
            if pixels[offset_left] & COL_MASK_BORDER == 0 {
                let offset_right = 3 * (WND_WIDTH * y + x + plus_min_dst);
                if pixels[offset_right] & COL_MASK_BORDER == 0 {
                    continue;
                }
            }
            if top == usize::max_value() {
                top = y;
            }
            bottom = y;
        }
        if bottom - top >= height_limit {
            continue;
        }
        for y in 0..max {
            let offset = 3 * (WND_WIDTH * y + x);
            if pixels[offset] & COL_MASK_MATERIAL != 0 {
                pixels[offset] |= COL_MASK_JAG;
            }
        }
    }
}

// Find top-left and bottom-left corners and return delta x between them
fn find_corners_delta(pixels: &mut Vec<u8>, max: usize) -> usize {

    let mut best_x: usize = WND_WIDTH;
    let mut best_y: usize = WND_HEIGHT;
    let mut best_dst = usize::max_value();

    let mut best_bot_x: usize = WND_WIDTH;
    let mut best_bot_y: usize = 0;
    let mut best_bot_dst = usize::max_value();

    for y in 0..max {
        for x in 0..max {
            let offset = 3 * (WND_WIDTH * y + x);
            let pix = pixels[offset];
            if pix & COL_MASK_BORDER == 0 || pix & COL_MASK_JAG != 0 {
                continue;
            }
            let dx = x;
            let dy = y;
            let dst = dx * dx + dy * dy;

            if dst < best_dst {
                best_x = x;
                best_y = y;
                best_dst = dst;
            }

            let bx = x;
            let by = max - y;
            let bst = bx * bx + by * by;

            if bst < best_bot_dst {
                best_bot_x = x;
                best_bot_y = y;
                best_bot_dst = bst;
            }
        }
    }

    // Draw them
    for x in 0..best_x + 1 {
        let offset = 3 * (WND_WIDTH * best_y + x);
        pixels[offset] = 0;
        pixels[offset + 1] = 255;
        pixels[offset + 2] = 0;
    }
    for y in 0..best_y + 1 {
        let offset = 3 * (WND_WIDTH * y + best_x);
        pixels[offset] = 0;
        pixels[offset + 1] = 255;
        pixels[offset + 2] = 0;
    }
    for x in 0..best_bot_x + 1 {
        let offset = 3 * (WND_WIDTH * best_bot_y + x);
        pixels[offset] = 255;
        pixels[offset + 2] = 0;
    }
    for y in 0..max as usize {
        if y >= WND_HEIGHT {
            break;
        }
        let offset = 3 * (WND_WIDTH * y + best_bot_x);
        pixels[offset] = 255;
        pixels[offset + 2] = 0;
    }

    return cmp::max(best_x, best_bot_x) - cmp::min(best_x, best_bot_x);
}

fn rotate_and_find_corners_delta(renderer: &mut Renderer,
                                 texture: &Texture,
                                 angle: f64,
                                 shift: usize,
                                 sqr: usize,
                                 width: u32,
                                 height: u32)
                                 -> (usize, Vec<u8>) {

    println!("angle={}", angle);

    renderer.clear();
    renderer.copy_ex(&texture,
                 None,
                 Some(Rect::new(shift as i32, shift as i32, width, height)),
                 angle,
                 None,
                 false,
                 false)
        .unwrap();

    //renderer.present();

    let mut pixels =
        renderer.read_pixels(Some(Rect::new(0, 0, WND_WIDTH as u32, WND_HEIGHT as u32)),
                         PixelFormatEnum::RGB24)
            .unwrap();

    // Detect piece and bounds
    let mut bounds = URect{min_x:usize::max_value(), min_y: usize::max_value(), max_x: 0, max_y: 0};
    for y in 0..sqr {
        for x in 0..sqr {
            if !detect_material(&mut pixels, x, y) {
                continue;
            }
            bounds.min_x = cmp::min(x, bounds.min_x);
            bounds.min_y = cmp::min(y, bounds.min_y);
            bounds.max_x = cmp::max(x, bounds.max_x);
            bounds.max_y = cmp::max(y, bounds.max_y);
        }
    }

    // Add one more so that we dont have to write ..max+1 everywhere
    bounds.max_x += 1;
    bounds.max_y += 1;

    // Detect borders
    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            detect_border(&mut pixels, x, y);
        }
    }

    // Find jags that could spoil finding corners
    detect_jags(&mut pixels, sqr, sqr / 32, sqr / 6, sqr / 6);

    return (find_corners_delta(&mut pixels, sqr), pixels);
}

fn main() {

    let sdl_context = sdl2::init().unwrap();

    let video_subsystem = sdl_context.video().unwrap();

    let window =
        video_subsystem.window("rust-sdl2 demo: Video", WND_WIDTH as u32, WND_HEIGHT as u32)
            .position_centered()
            .opengl()
            .build()
            .unwrap();

    let mut renderer = window.renderer().build().unwrap();
    let texture = renderer.load_texture("2.jpg").unwrap();

    let TextureQuery { width, height, .. } = texture.query();

    println!("{}x{}", width, height);

    // Some space so that rotation does not crop image
    let shift = cmp::max(width, height) / 3 + 1;

    // Squate that the puzzle always fits
    let sqr = (5 * shift) as usize; // 1xleft shift, 3/3 texture, 1xright shift

    for side in 0..4 {

        let mut best_corner_delta = usize::max_value();
        let mut best_corner_angle = 0;

        'rotating: for r in -10..11 {

            let angle = 90 * side + r;
            println!("angle={}", angle);

            let rv = rotate_and_find_corners_delta(&mut renderer,
                                                   &texture,
                                                   angle as f64,
                                                   shift as usize,
                                                   sqr,
                                                   width,
                                                   height);
            let corner_delta = rv.0;
            let pixels = rv.1;

            println!("corner_delta={}", corner_delta);
            if corner_delta < best_corner_delta {
                best_corner_delta = corner_delta;
                best_corner_angle = angle;
            }

            let mut res_texture = renderer.create_texture_streaming(PixelFormatEnum::RGB24,
                                          WND_WIDTH as u32,
                                          WND_HEIGHT as u32)
                .unwrap();

            // Create texture with result
            let mut index = 0;
            res_texture.with_lock(None,
                           |buffer: &mut [u8], pitch: usize| for y in 0..WND_HEIGHT {
                               for x in 0..WND_WIDTH {
                                   let offset = y * pitch + x * 3;
                                   buffer[offset + 0] = pixels[offset];
                                   buffer[offset + 1] = pixels[offset + 1];
                                   buffer[offset + 2] = pixels[offset + 2];
                                   index += 1;
                               }
                           })
                .unwrap();

            renderer.clear();
            renderer.copy(&res_texture, None, None).unwrap();
            renderer.present();


            let mut event_pump = sdl_context.event_pump().unwrap();

            'running: loop {
                for event in event_pump.poll_iter() {
                    match event {
                        Event::KeyDown { keycode: Some(Keycode::R), .. } => break 'running,
                        Event::Quit { .. } |
                        Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break 'rotating,
                        _ => {}
                    }
                }
                // The rest of the game loop goes here...
            }
        }

        println!("best_corner_angle={} best_corner_delta={}",
                 best_corner_angle,
                 best_corner_delta);
    }
}
