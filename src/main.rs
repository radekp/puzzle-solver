extern crate sdl2;
extern crate image;

use std::fs;
use std::cmp;
use std::fs::File;
use std::path::Path;
use std::str::FromStr;
use std::error::Error;
use std::cmp::Ordering;
use std::io::prelude::*;

use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::event::Event;
use sdl2::pixels::Color;
use sdl2::keyboard::Keycode;
use sdl2::image::LoadTexture;
use sdl2::render::TextureQuery;
use sdl2::render::Renderer;
use sdl2::render::Texture;

// SDL window size - puzzle pieces bitmap must fit even with rotation
const WND_WIDTH: usize = 1000;
const WND_HEIGHT: usize = 1000;

// Color masks used to detect borders etc...
const RED_MASK_NO_MATERIAL: u8 = 1;
const RED_MASK_MATERIAL: u8 = 1 << 6;
const RED_MASK_BORDER: u8 = 1 << 7;
const RED_MASK_JAG: u8 = 1 << 5;
const RED_MASK_FLOOD_FILLED: u8 = 1 << 1;

#[derive(Copy, Clone)]
struct URect {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

#[derive(Copy, Clone)]
struct DisplayPixelState {
    autorotate: bool,
}

struct EdgeInfo {
    points: Vec<(usize, usize)>,
    distances: Vec<usize>,
    edge_no: usize, // e.g. 103 is 10.3.txt
    max_x: usize,
    max_y: usize,
}

/*
// Near point iterator
// Iterates points in spiral centered at cx,cy
//
//     9 ....
//       1 2 3
//       8   4
//       7 6 5
//
// If a==0 it will start in cx,cy orherwise a is square side on start
fn near_iter_begin(cx: usize, cy: usize, start_a: usize) -> (usize, usize, usize) {
    return (cx - start_a as usize, cy - start_a as usize, start_a);
}

// Return next point in spiral
fn near_iter_next(cx: usize,
                  cy: usize,
                  prev_x: usize,
                  prev_y: usize,
                  prev_a: usize)
                  -> (usize, usize, usize) {

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
}*/

enum FFMode {
    FourWay,
    EightWay,
}

fn flood_fill(pixels: &mut Vec<u8>,
              sqr: usize,
              bounds: URect,
              x: usize,
              y: usize,
              ff_mode: FFMode,
              compare_red_mask: u8)
              -> usize {

    let mut src = vec![(x, y)];
    let mut dst = vec![];
    let mut res = 0;
    loop {

        for p in src.iter() {
            if p.0 < bounds.min_x || p.0 > bounds.max_x || p.1 < bounds.min_y ||
               p.1 > bounds.max_y {
                continue;
            }
            let offset = 3 * (sqr * p.1 + p.0);
            let pix = pixels[offset];
            if pix & compare_red_mask == 0 {
                continue;
            }
            if pix & RED_MASK_FLOOD_FILLED != 0 {
                continue;
            }
            pixels[offset] |= RED_MASK_FLOOD_FILLED;
            res += 1;

            dst.push((p.0 - 1, p.1));
            dst.push((p.0 + 1, p.1));
            dst.push((p.0, p.1 - 1));
            dst.push((p.0, p.1 + 1));

            match ff_mode {
                FFMode::EightWay => {
                    dst.push((p.0 - 1, p.1 - 1));
                    dst.push((p.0 + 1, p.1 - 1));
                    dst.push((p.0 - 1, p.1 + 1));
                    dst.push((p.0 + 1, p.1 + 1));
                }
                _ => {}
            }
        }
        if dst.len() == 0 {
            return res;
        }
        src.clear();
        let tmp = src;
        src = dst;
        dst = tmp;
    }
}

fn flood_unfill(pixels: &mut Vec<u8>, sqr: usize, bounds: URect) {
    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            pixels[3 * (sqr * y + x)] &= !RED_MASK_FLOOD_FILLED;
        }
    }
}

/*fn flood_col(pixels: &mut Vec<u8>, sqr: usize, bounds: URect, r: u8, g: u8, b: u8) {
    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            let offset = 3 * (sqr * y + x);
            if pixels[offset] & RED_MASK_FLOOD_FILLED == 0 {
                continue;
            }
            pixels[offset] = r;
            pixels[offset + 1] = g;
            pixels[offset + 2] = b;
        }
    }
}*/

fn flood_points(pixels: &mut Vec<u8>, sqr: usize, bounds: URect) -> Vec<(usize, usize)> {
    let mut res = vec![];
    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            if pixels[3 * (sqr * y + x)] & RED_MASK_FLOOD_FILLED != 0 {
                res.push((x, y));
            }
        }
    }
    return res;
}

// Detect piece color - in my case they are dark blue
fn detect_material(pixels: &mut Vec<u8>, sqr: usize) -> URect {

    let mut bounds = URect {
        min_x: usize::max_value(),
        min_y: usize::max_value(),
        max_x: 0,
        max_y: 0,
    };

    // Check each pixel color, compare with treshold and repaint wit material/no material color
    for y in 0..sqr {
        for x in 0..sqr {
            let offset = 3 * (sqr * y + x);
            let r = pixels[offset] as i32;
            let g = pixels[offset + 1] as i32;
            let b = pixels[offset + 2] as i32;
            if r + g + b > 3 * 127 {
                pixels[offset] = RED_MASK_NO_MATERIAL;
                pixels[offset + 1] = 0;
                pixels[offset + 2] = 0;
                continue;
            }
            pixels[offset] = RED_MASK_MATERIAL;
            pixels[offset + 1] = RED_MASK_MATERIAL;
            pixels[offset + 2] = RED_MASK_MATERIAL;

            bounds.min_x = cmp::min(x, bounds.min_x);
            bounds.min_y = cmp::min(y, bounds.min_y);
            bounds.max_x = cmp::max(x, bounds.max_x);
            bounds.max_y = cmp::max(y, bounds.max_y);
        }
    }

    // More space so that we dont have to write ..max+1 everywhere and 1pixel so that flood fill
    // works.
    bounds.min_x -= 1;
    bounds.min_y -= 1;
    bounds.max_x += 2;
    bounds.max_y += 2;

    // Flood fill from top-left corner - no material should be there
    flood_fill(pixels,
               sqr,
               bounds,
               bounds.min_x,
               bounds.min_y,
               FFMode::FourWay,
               RED_MASK_NO_MATERIAL);

    // Paint not filled pixels with material color. This fills holes inside of shapes.
    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            let offset = 3 * (sqr * y + x);
            if pixels[offset] & RED_MASK_FLOOD_FILLED == 0 {
                pixels[offset] |= RED_MASK_MATERIAL;
            }
        }
    }

    return bounds;
}

// Picks the biggest piece, removing small ones
fn detect_piece(pixels: &mut Vec<u8>, sqr: usize, bounds: URect) {

    flood_unfill(pixels, sqr, bounds);

    let mut best_x = bounds.min_x;
    let mut best_y = bounds.min_y;
    let mut best_count = 0;

    // Flood fill all material and count number of filled
    for y in 0..sqr {
        for x in 0..sqr {
            let pix = pixels[3 * (sqr * y + x)];
            if pix & RED_MASK_MATERIAL == 0 || pix & RED_MASK_FLOOD_FILLED != 0 {
                continue;
            }
            let count = flood_fill(pixels,
                                   sqr,
                                   bounds,
                                   x,
                                   y,
                                   FFMode::FourWay,
                                   RED_MASK_MATERIAL);

            if count < best_count {
                continue;
            }
            best_count = count;
            best_x = x;
            best_y = y;
        }
    }

    // Fill the largest material
    flood_unfill(pixels, sqr, bounds);
    flood_fill(pixels,
               sqr,
               bounds,
               best_x,
               best_y,
               FFMode::FourWay,
               RED_MASK_MATERIAL);

    // And remove the rest
    for y in 0..sqr {
        for x in 0..sqr {
            let offset = 3 * (sqr * y + x);
            let pix = pixels[offset];
            if pix & RED_MASK_MATERIAL != 0 && pix & RED_MASK_FLOOD_FILLED == 0 {
                pixels[offset] &= !RED_MASK_MATERIAL;
            }
        }
    }
}

// Draw border pixels with RED_MASK_BORDER
fn detect_border(pixels: &mut Vec<u8>, sqr: usize, bounds: URect) {

    // Border is material that touches flood filled
    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            let offset = 3 * (sqr * y + x);
            if pixels[offset] & RED_MASK_MATERIAL == 0 {
                // not material, skip
                continue;
            }
            if pixels[offset-3] & RED_MASK_NO_MATERIAL == 0     // no materi must be near
                && pixels[offset+3] & RED_MASK_NO_MATERIAL == 0 &&
               pixels[offset + 3 * sqr] & RED_MASK_NO_MATERIAL == 0 &&
               pixels[offset - 3 * sqr] & RED_MASK_NO_MATERIAL == 0 {
                continue;
            }
            pixels[offset] |= RED_MASK_BORDER;
        }
    }
}

fn count_no_border_mat(pixels: &mut Vec<u8>, sqr: usize, x: usize, y: usize) -> usize {
    let pix = pixels[3 * (sqr * y + x)];
    if pix & RED_MASK_MATERIAL == 0 || pix & RED_MASK_BORDER != 0 {
        return 0;
    }
    return 1;
}

// Removes dead end nipples from border
//
//    X     <- removes this
//    X     <- and this
// XXXXXXXX <- border
// MMMMMMMM <- material
fn remove_dead_end_border(pixels: &mut Vec<u8>, sqr: usize, bounds: URect) {
    loop {
        let mut count = 0;
        for y in bounds.min_y..bounds.max_y {
            for x in bounds.min_x..bounds.max_x {
                let offset = 3 * (sqr * y + x);
                if pixels[offset] & RED_MASK_BORDER == 0 {
                    continue;
                }
                // Check point left, right, up and down
                let near_count = count_no_border_mat(pixels, sqr, x + 1, y) +
                                 count_no_border_mat(pixels, sqr, x - 1, y) +
                                 count_no_border_mat(pixels, sqr, x, y + 1) +
                                 count_no_border_mat(pixels, sqr, x, y - 1);

                if near_count == 0 {
                    pixels[offset] = 0; // not border and not material now
                    count += 1;
                }
            }
        }
        //println!("remove_dead_end_border count={}", count);
        if count == 0 {
            return;
        }
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
               sqr: usize,
               bounds: URect,
               plus_min_dst: usize,
               width_limit: usize,
               height_limit: usize) {

    // Foreach row
    for y in bounds.min_y..bounds.max_y {
        // Compute left and right coordinate
        let mut left = usize::max_value();
        let mut right = usize::max_value();
        for x in bounds.min_x..bounds.max_x {
            let offset_up = 3 * (sqr * (y - plus_min_dst) + x);
            if pixels[offset_up] & RED_MASK_BORDER == 0 {
                let offset_down = 3 * (sqr * (y + plus_min_dst) + x);
                if pixels[offset_down] & RED_MASK_BORDER == 0 {
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
        for x in bounds.min_x..bounds.max_x {
            let offset = 3 * (sqr * y + x);
            if pixels[offset] & RED_MASK_MATERIAL != 0 {
                pixels[offset] |= RED_MASK_JAG;
            }
        }
    }

    // Same for columns
    for x in bounds.min_x..bounds.max_x {
        let mut top = usize::max_value();
        let mut bottom = usize::max_value();
        for y in bounds.min_y..bounds.max_y {
            let offset_left = 3 * (sqr * y + x - plus_min_dst);
            if pixels[offset_left] & RED_MASK_BORDER == 0 {
                let offset_right = 3 * (sqr * y + x + plus_min_dst);
                if pixels[offset_right] & RED_MASK_BORDER == 0 {
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
        for y in bounds.min_y..bounds.max_y {
            let offset = 3 * (sqr * y + x);
            if pixels[offset] & RED_MASK_MATERIAL != 0 {
                pixels[offset] |= RED_MASK_JAG;
            }
        }
    }
}

// Find top-left and bottom-left corners and return delta x between them
fn find_corners(pixels: &mut Vec<u8>,
                sqr: usize,
                bounds: URect,
                draw_corners: bool)
                -> (usize, usize, usize, usize) {

    let mut best_x: usize = sqr;
    let mut best_y: usize = sqr;
    let mut best_dst = usize::max_value();

    let mut best_bot_x: usize = sqr;
    let mut best_bot_y: usize = 0;
    let mut best_bot_dst = usize::max_value();

    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            let offset = 3 * (sqr * y + x);
            let pix = pixels[offset];
            if pix & RED_MASK_BORDER == 0 || pix & RED_MASK_JAG != 0 {
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
            let by = sqr - y;
            let bst = bx * bx + by * by;

            if bst < best_bot_dst {
                best_bot_x = x;
                best_bot_y = y;
                best_bot_dst = bst;
            }
        }
    }

    if draw_corners {

        for x in 0..best_x + 1 {
            let offset = 3 * (sqr * best_y + x);
            pixels[offset] = 0;
            pixels[offset + 1] = 255;
            pixels[offset + 2] = 0;
        }
        for y in 0..best_y + 1 {
            let offset = 3 * (sqr * y + best_x);
            pixels[offset] = 0;
            pixels[offset + 1] = 255;
            pixels[offset + 2] = 0;
        }
        for x in 0..best_bot_x + 1 {
            let offset = 3 * (sqr * best_bot_y + x);
            pixels[offset] = 255;
            pixels[offset + 2] = 0;
        }
        for y in 0..sqr as usize {
            if y >= sqr {
                break;
            }
            let offset = 3 * (sqr * y + best_bot_x);
            pixels[offset] = 255;
            pixels[offset + 2] = 0;
        }
    }

    return (best_x, best_y, best_bot_x, best_bot_y);
}

fn rotate_and_find_corners(renderer: &mut Renderer,
                           texture: &Texture,
                           angle: f64,
                           shift: usize,
                           sqr: usize,
                           width: u32,
                           height: u32,
                           draw_corners: bool)
                           -> (usize, usize, usize, usize, Vec<u8>, URect) {

    renderer.set_draw_color(Color::RGB(255, 255, 255));
    renderer.fill_rect(Rect::new(0, 0, sqr as u32, sqr as u32)).unwrap();

    renderer.copy_ex(&texture,
                     None,
                     Some(Rect::new(shift as i32, shift as i32, width, height)),
                     angle,
                     None,
                     false,
                     false)
        .unwrap();

    //renderer.present();

    let mut pixels = renderer.read_pixels(Some(Rect::new(0, 0, sqr as u32, sqr as u32)),
                                          PixelFormatEnum::RGB24)
        .unwrap();

    // Detect material and bounds
    let bounds = detect_material(&mut pixels, sqr);

    // Detect pieces (the biggest pieces of material)
    detect_piece(&mut pixels, sqr, bounds);

    // Detect borders
    detect_border(&mut pixels, sqr, bounds);

    // Remove dead end points from border
    remove_dead_end_border(&mut pixels, sqr, bounds);

    // Find jags that could spoil finding corners
    detect_jags(&mut pixels, sqr, bounds, sqr / 32, sqr / 6, sqr / 6);

    let rv = find_corners(&mut pixels, sqr, bounds, draw_corners);

    return (rv.0, rv.1, rv.2, rv.3, pixels, bounds);
}

fn find_edge(pixels: &mut Vec<u8>,
             sqr: usize,
             bounds: URect,
             top_x: usize,
             top_y: usize,
             bot_x: usize,
             bot_y: usize)
             -> String {

    // Split border in top and bot points into 2 parts
    for i in 0..10 {
        pixels[3 * (sqr * (top_y - i) + top_x - i)] &= !RED_MASK_BORDER;
        pixels[3 * (sqr * (top_y + i) + top_x + i)] &= !RED_MASK_BORDER;
        pixels[3 * (sqr * (bot_y + i) + bot_x - i)] &= !RED_MASK_BORDER;
        pixels[3 * (sqr * (bot_y - i) + bot_x + i)] &= !RED_MASK_BORDER;

        pixels[3 * (sqr * (top_y - i) + top_x - i) + 1] = (255 - 25 * i) as u8;
        pixels[3 * (sqr * (top_y + i) + top_x + i) + 1] = (255 - 25 * i) as u8;
        pixels[3 * (sqr * (bot_y + i) + bot_x - i) + 1] = (255 - 25 * i) as u8;
        pixels[3 * (sqr * (bot_y - i) + bot_x + i) + 1] = (255 - 25 * i) as u8;
    }

    // Fill the edge in the middle of piece height
    flood_unfill(pixels, sqr, bounds);
    let y = (bounds.min_y + bounds.max_y) / 2;
    for x in bounds.min_x..bounds.max_x {
        if pixels[3 * (sqr * y + x)] & RED_MASK_BORDER == 0 {
            continue;
        }
        flood_fill(pixels, sqr, bounds, x, y, FFMode::EightWay, RED_MASK_BORDER);
        break;
    }

    let mut edge = flood_points(pixels, sqr, bounds);

    draw_coords(pixels, sqr, &edge, 0, 0, 0, 0, 255);

    // Find min
    let mut min_x = usize::max_value();
    let mut min_y = usize::max_value();
    for p in edge.iter() {
        min_x = cmp::min(p.0, min_x);
        min_y = cmp::min(p.1, min_y);
    }

    // Sort edge by y and then by x, so that max_x,max_y is last so that
    // compare can be fast
    edge.sort_by(|a, b| (a.1 * sqr + a.0).cmp(&(b.1 * sqr + b.0)));

    let mut res: String = "".to_string();
    for p in edge.iter() {
        if res.len() > 0 {
            res += "\n";
        }
        res = res + &format!("{},{}", p.0 - min_x, p.1 - min_y);
    }
    res
}

enum UserAction {
    Rotate,
    Quit,
    Solve,
}

fn display_pixels(pixels: &Vec<u8>,
                  sqr: usize,
                  sdl_context: &sdl2::Sdl,
                  renderer: &mut Renderer,
                  state: &mut DisplayPixelState)
                  -> UserAction {

    let mut res_texture =
        renderer.create_texture_streaming(PixelFormatEnum::RGB24, sqr as u32, sqr as u32).unwrap();

    // Create texture with result
    let mut index = 0;
    res_texture.with_lock(None, |buffer: &mut [u8], pitch: usize| for y in 0..sqr {
            for x in 0..sqr {
                let src_offset = y * pitch + x * 3;
                let dst_offset = y * pitch + x * 3;
                buffer[dst_offset + 0] |= pixels[src_offset];
                buffer[dst_offset + 1] |= pixels[src_offset + 1];
                buffer[dst_offset + 2] |= pixels[src_offset + 2];
                index += 1;
            }
        })
        .unwrap();


    if state.autorotate {
        renderer.clear();
        renderer.copy(&res_texture, None, None).unwrap();
        renderer.present();
        return UserAction::Rotate;
    }

    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut dst_rect = Rect::new(0, 0, sqr as u32, sqr as u32);

    loop {
        for event in event_pump.poll_iter() {
            renderer.clear();
            renderer.copy(&res_texture, None, Some(dst_rect)).unwrap();
            renderer.present();
            match event {
                Event::KeyDown { keycode: Some(Keycode::R), .. } => return UserAction::Rotate,
                Event::KeyDown { keycode: Some(Keycode::P), .. } => {
                    let w = dst_rect.width();
                    let h = dst_rect.height();
                    dst_rect.set_width(w * 2);
                    dst_rect.set_height(h * 2);
                }
                Event::KeyDown { keycode: Some(Keycode::M), .. } => {
                    let w = dst_rect.width();
                    let h = dst_rect.height();
                    dst_rect.set_x(0);
                    dst_rect.set_y(0);
                    dst_rect.set_width(w / 2);
                    dst_rect.set_height(h / 2);
                }
                Event::KeyDown { keycode: Some(Keycode::Right), .. } => {
                    let x = dst_rect.x();
                    let step = (dst_rect.width() / 10) as i32;
                    dst_rect.set_x(x - step);
                }
                Event::KeyDown { keycode: Some(Keycode::Left), .. } => {
                    let x = dst_rect.x();
                    let step = (dst_rect.width() / 10) as i32;
                    dst_rect.set_x(x + step);
                }
                Event::KeyDown { keycode: Some(Keycode::Down), .. } => {
                    let y = dst_rect.y();
                    let step = (dst_rect.height() / 10) as i32;
                    dst_rect.set_y(y - step);
                }
                Event::KeyDown { keycode: Some(Keycode::Up), .. } => {
                    let y = dst_rect.y();
                    let step = (dst_rect.height() / 10) as i32;
                    dst_rect.set_y(y + step);
                }
                Event::KeyDown { keycode: Some(Keycode::A), .. } => {
                    state.autorotate = !state.autorotate;
                    return UserAction::Rotate;
                }
                Event::KeyDown { keycode: Some(Keycode::S), .. } => {
                    return UserAction::Solve;
                }
                Event::Quit { .. } |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => return UserAction::Quit,
                _ => {}
            }
        }
        // The rest of the game loop goes here...
    }
}

fn process_png(img_file: &str,
               png_no: usize,
               sdl_context: &sdl2::Sdl,
               display_state: &mut DisplayPixelState) {

    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window(img_file, WND_WIDTH as u32, WND_HEIGHT as u32)
        .position(200, 0)
        .opengl()
        .build()
        .unwrap();

    let mut renderer = window.renderer().build().unwrap();

    let texture = renderer.load_texture(img_file).unwrap();

    let TextureQuery { width, height, .. } = texture.query();

    // Some space so that rotation does not crop image. Must be multiple of 4
    // to play well with texture pitch.
    let shift = ((cmp::max(width, height) as usize) / 3 + 5) & !3usize;

    // Squate that the shifted puzzle always fits
    let sqr = (5 * shift) as usize; // 1xleft shift, 3/3 texture, 1xright shift

    println!("{} {}x{} shift={} sqr={}",
             img_file,
             width,
             height,
             shift,
             sqr);

    let wnd_size = renderer.window().unwrap().size();
    if sqr >= wnd_size.0 as usize || sqr >= wnd_size.1 as usize {
        panic!("{} too big {}x{} window is just {}x{}",
               img_file,
               sqr,
               sqr,
               wnd_size.0,
               wnd_size.1);
    }

    // Resize window
    /*renderer.window_mut()
        .unwrap()
        .set_size(sqr as u32, sqr as u32)
        .unwrap();*/

    for side in 0..4 {

        let mut best_corner_delta = usize::max_value();
        let mut best_corner_angle = 0f64;

        let mut r = -10f64;
        'rotating: loop {

            let angle = (90 * side) as f64 + r;
            //println!("angle={}", angle);

            let rv = rotate_and_find_corners(&mut renderer,
                                             &texture,
                                             angle,
                                             shift as usize,
                                             sqr,
                                             width,
                                             height,
                                             true);

            let top_x = rv.0;
            let bot_x = rv.2;
            let pixels = rv.4;

            let corner_delta = cmp::max(top_x, bot_x) - cmp::min(top_x, bot_x);

            //println!("corner_delta={}", corner_delta);
            if corner_delta < best_corner_delta {
                best_corner_delta = corner_delta;
                best_corner_angle = angle;
            }

            match display_pixels(&pixels, sqr, sdl_context, &mut renderer, display_state) {
                UserAction::Quit => break 'rotating,
                _ => {}
            }

            r += 0.2f64;
            if r > 10f64 {
                break;
            }
        }

        println!("best_corner_angle={}", best_corner_angle);

        let rv = rotate_and_find_corners(&mut renderer,
                                         &texture,
                                         best_corner_angle as f64,
                                         shift as usize,
                                         sqr,
                                         width,
                                         height,
                                         false);

        let top_x = rv.0;
        let top_y = rv.1;
        let bot_x = rv.2;
        let bot_y = rv.3;
        let mut pixels = rv.4;
        let bounds = rv.5;

        // Save left edge coordinates to file
        let content = find_edge(&mut pixels, sqr, bounds, top_x, top_y, bot_x, bot_y);
        let filename = format!("{}.txt", 4 * png_no + side);
        let txt_path = Path::new(img_file).with_file_name(filename);
        let display = txt_path.display();

        let mut file = match File::create(&txt_path) {
            Err(why) => panic!("couldn't create {}: {}", display, why.description()),
            Ok(file) => file,
        };

        match file.write_all(content.as_bytes()) {
            Err(why) => panic!("couldn't write to {}: {}", display, why.description()),
            Ok(_) => println!("successfully wrote to {}", display),
        }

        // Make .done file so that we can detect processed pngs
        if side == 3 {
            write_done_file(img_file);
        }

        display_pixels(&pixels, sqr, sdl_context, &mut renderer, display_state);
    }
}

fn read_txt(txt_file: &str) -> Vec<(usize, usize)> {

    // Create a path to the desired file
    let path = Path::new(txt_file);
    let display = path.display();

    // Open the path in read-only mode, returns `io::Result<File>`
    let mut file = match File::open(&path) {
        // The `description` method of `io::Error` returns a string that
        // describes the error
        Err(why) => panic!("couldn't open {}: {}", display, why.description()),
        Ok(file) => file,
    };

    // Read the file contents into a string, returns `io::Result<usize>`
    let mut content = String::new();
    match file.read_to_string(&mut content) {
        Err(why) => panic!("couldn't read {}: {}", display, why.description()),
        Ok(_) => println!("{} loaded", display),
    }

    let mut coords = vec![];
    for line in content.split('\n') {
        let v: Vec<&str> = line.split(',').collect();
        coords.push((usize::from_str(v[0]).unwrap(), usize::from_str(v[1]).unwrap()));
    }

    return coords;
}

fn draw_coords(pixels: &mut Vec<u8>,
               sqr: usize,
               coords: &Vec<(usize, usize)>,
               left: usize,
               top: usize,
               r: u8,
               g: u8,
               b: u8) {
    for p in coords {
        let x = p.0 + left;
        let y = p.1 + top;
        let offset = 3 * (sqr * y + x);
        pixels[offset] = r;
        pixels[offset + 1] = g;
        pixels[offset + 2] = b;
    }
}

fn flip_coords(coords: &Vec<(usize, usize)>) -> Vec<(usize, usize)> {

    let mut max_x = 0;
    let mut max_y = 0;

    for p in coords {
        max_x = cmp::max(p.0, max_x);
        max_y = cmp::max(p.1, max_y);
    }

    let mut res = vec![];
    for p in coords {
        res.push((max_x - p.0, max_y - p.1));
    }
    res.reverse();
    return res;
}

fn compare_edge_info(a: &EdgeInfo, b: &EdgeInfo) -> Ordering {
    return a.max_y.cmp(&b.max_y);
}

fn compare_edges(edges: &mut Vec<EdgeInfo>, a: usize, b: usize, sqr: usize, rec: bool) -> usize {

    let mut res = 0;
    let mut new_distances = vec![];

    {
        let ref edge_a = edges[a];
        let ref edge_b = edges[b];
        let ref points_a = edge_a.points;
        let ref points_b = edge_b.points;
        let ref distances_a = edge_a.distances;

        for point_b in points_b {
            // point b must be flipped
            let b = (edge_b.max_x - point_b.0, edge_b.max_y - point_b.1);
            let offset = sqr * b.1 + b.0;
            let mut best_dst = distances_a[offset];
            if best_dst == usize::max_value() {
                for a in points_a {
                    let dx = (a.0 as isize) - (b.0 as isize);
                    let dy = (a.1 as isize) - (b.1 as isize);
                    let dst = (dx * dx + dy * dy) as usize;
                    if dst < best_dst {
                        best_dst = dst;
                    }
                }
                new_distances.push((offset, best_dst));
            }
            res += best_dst;
        }
    }

    // Fill up newly computed distances
    for d in new_distances {
        edges[a].distances[d.0] = d.1;
    }

    if rec {
        res += compare_edges(edges, a, b, sqr, false); // the same but compute dst from b to a
    }
    return res;
}

// Make file processed
fn write_done_file(path: &str) {
    let done_path = Path::new(path).with_extension("done");
    match File::create(&done_path) {
        Err(why) => {
            panic!("couldn't create {}: {}",
                   done_path.display(),
                   why.description())
        }
        Ok(file) => file,
    };
}

// Is file already processed?
fn is_done(path: &str) -> bool {
    let done_path = Path::new(&path).with_extension("done");
    if !done_path.exists() {
        return false;
    }
    println!("skipping {} because {} exists", path, done_path.display());

    return true;
}

fn main() {

    let sdl_context = sdl2::init().unwrap();

    let mut display_state = DisplayPixelState { autorotate: false };

    // Process all .png files - this will write 4 txt files for each edge
    let entries = fs::read_dir("./data").unwrap();
    for entry in entries {
        //println!("Name: {}", path.unwrap().path().into_os_string().into_string());

        let path = entry.unwrap().path();
        if path.extension().unwrap() != "png" {
            continue;
        }
        let png_no: usize = path.file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap();

        let path_str = path.into_os_string().into_string().unwrap();

        if is_done(&path_str) {
            continue;
        }

        process_png(&path_str, png_no, &sdl_context, &mut display_state);
    }
    //process_png("9.png", &sdl_context);

    // Read txt files with edges
    let mut edges = vec![];
    let entries = fs::read_dir("./data").unwrap();
    for entry in entries {

        let path = entry.unwrap().path();
        if path.extension().unwrap() != "txt" {
            continue;
        }

        let edge_no: usize = path.file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .replace(".", "")
            .parse()
            .unwrap();

        let path_str = path.into_os_string().into_string().unwrap();

        // Skip edges that are already solved
        if is_done(&path_str) {
            continue;
        }

        let points = read_txt(&path_str);

        // Compute height
        let mut max_x = 0;
        let mut max_y = 0;
        for p in points.iter() {
            max_x = cmp::max(max_x, p.0);
            max_y = cmp::max(max_y, p.1);
        }

        let edge_info = EdgeInfo {
            points: points,
            distances: vec![],
            edge_no: edge_no,
            max_x: max_x,
            max_y: max_y,
        };
        edges.push(edge_info);
    }

    // Max x and y in all edges
    let mut max_x = 0;
    let mut max_y = 0;
    for edge in edges.iter() {
        max_x = cmp::max(max_x, edge.max_x);
        max_y = cmp::max(max_y, edge.max_y);
    }
    let width = max_x + 1;
    let height = max_y + 1;

    // SDL window - make it modulo 4 to play well with texture pitch
    let sqr = cmp::max(width, height) + 5 & !3usize;

    // Make up distances table for fast nearest edge searching
    for edge in edges.iter_mut() {
        edge.distances = vec![usize::max_value(); sqr * sqr];
    }

    let mut pixels: Vec<u8> = vec![0;3*sqr*sqr];
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("puzzle solver", sqr as u32, sqr as u32)
        .position(0, 0)
        .opengl()
        .build()
        .unwrap();

    let mut renderer = window.renderer().build().unwrap();

    // Sort edges by max y
    edges.sort_by(|a, b| compare_edge_info(a, b));

    // Compare edges - start with near edges (they have similar height)
    let mut cmp_results = vec![];
    for d in 1..cmp::min(edges.len(), 128) {
        let mut best_score = usize::max_value();
        for i in 0..(edges.len() - d) {

            let j = i + d;

            let score = compare_edges(&mut edges, i, j, sqr, true);

            let ref edge_i = edges[i];
            let ref edge_j = edges[i + d];

            cmp_results.push((score, i, j));

            // Display best score so that some progress is shown
            if score < best_score {
                best_score = score;
            } else if display_state.autorotate {
                continue;
            }
            println!("red {:>4}.{} vs green {:>4}.{} score={:>12}, todo i={} d={}/{}",
                     edge_i.edge_no >> 2,
                     edge_i.edge_no & 3,
                     edge_j.edge_no >> 2,
                     edge_j.edge_no & 3,
                     score,
                     i,
                     d,
                     edges.len());

            // Display points_i with red, points_j in green
            draw_coords(&mut pixels, sqr, &edge_i.points, 0, 0, 255, 0, 0);
            draw_coords(&mut pixels,
                        sqr,
                        &flip_coords(&edge_j.points),
                        0,
                        0,
                        0,
                        255,
                        0);

            display_pixels(&pixels,
                           sqr,
                           &sdl_context,
                           &mut renderer,
                           &mut display_state);

            for p in pixels.iter_mut() {
                *p = 0;
            }
        }
    }

    // Sort results by best score
    cmp_results.sort_by(|a, b| a.cmp(&b));

    println!("Compared edges:");

    display_state.autorotate = false;
    for r in cmp_results.iter() {

        //     J  <-  I
        let ref edge_i = edges[r.1];
        let ref edge_j = edges[r.2];

        let i_no = edge_i.edge_no;
        let j_no = edge_j.edge_no;

        println!("{:>4}.{}->{:<4}.{} score={:>12}, to solve press s",
                 i_no >> 2,
                 i_no & 3,
                 j_no >> 2,
                 j_no & 3,
                 r.0);

        // Find point M:
        //
        //     M
        //     ^
        //     |
        //     J  ->  I
        //
        let side_j_plus = (j_no + 1) & 3;

        //println!("  searching M sharing edge {}.{}", j_no >> 2, side_j_plus);
        for r2 in cmp_results.iter() {
            let ref edge_k = edges[r2.1];
            let ref edge_l = edges[r2.2];
            let k_no = edge_k.edge_no;
            let l_no = edge_l.edge_no;

            let m_no = if k_no >> 2 == j_no >> 2 && k_no & 3 == side_j_plus {
                l_no
            } else if l_no >> 2 == j_no >> 2 && l_no & 3 == side_j_plus {
                k_no
            } else {
                continue;
            };

            println!("        {}.{}     K={:>4}.{} vs L={:>4}.{} score J->M={:>12}",
                     m_no >> 2,
                     m_no & 3,
                     k_no >> 2,
                     k_no & 3,
                     l_no >> 2,
                     l_no & 3,
                     r2.0);

            // Find point P sharing border with M and J:
            //
            //     M  -> P
            //     ^     ^
            //     |     |
            //     J  -> I
            //
            let side_m_plus = (m_no + 1) & 3;
            //println!("  searching P sharing edge {}.{}", m_no >> 2, side_m_plus);
            for r3 in cmp_results.iter() {
                let ref edge_n = edges[r3.1];
                let ref edge_o = edges[r3.2];
                let n_no = edge_n.edge_no;
                let o_no = edge_o.edge_no;

                let p_no = if n_no >> 2 == m_no >> 2 && n_no & 3 == side_m_plus {
                    o_no
                } else if o_no >> 2 == m_no >> 2 && o_no & 3 == side_m_plus {
                    n_no
                } else {
                    continue;
                };

                println!("        {}.{}       N={:>4}.{} vs O={:>4}.{} score M->P={:>12}",
                         p_no >> 2,
                         p_no & 3,
                         n_no >> 2,
                         n_no & 3,
                         o_no >> 2,
                         o_no & 3,
                         r3.0);

                // Compare P with I
                let p_plus = ((p_no + 1) & 3) | (p_no & !3);
                let i_minus = ((i_no + 3) & 3) | (i_no & !3);
                /*println!("  searching for P->I edge {}.{} -> {}.{}",
                         p_plus >> 2,
                         p_plus & 3,
                         i_minus >> 2,
                         i_minus & 3);*/
                for r4 in cmp_results.iter() {
                    let q_no = edges[r4.1].edge_no;
                    let r_no = edges[r4.2].edge_no;

                    if (q_no == p_plus && r_no == i_minus) || (q_no == i_minus && r_no == p_plus) {

                         println!("        {}.{} -> {}.{} score={}, FINAL SCORE={}",
                                                p_plus >> 2,
			    p_plus & 3,
			    i_minus >> 2,
			    i_minus & 3,

                                 r4.0,
                                 r.0 + r2.0 + r3.0 + r4.0);
                        break;
                    }
                }
                break;
            }

            break;
        }


        for p in pixels.iter_mut() {
            *p = 0;
        }
        draw_coords(&mut pixels, sqr, &edge_i.points, 0, 0, 255, 0, 0);
        draw_coords(&mut pixels,
                    sqr,
                    &flip_coords(&edge_j.points),
                    0,
                    0,
                    0,
                    255,
                    0);

        match display_pixels(&pixels,
                             sqr,
                             &sdl_context,
                             &mut renderer,
                             &mut display_state) {
            UserAction::Solve => {
                /*write_done_file(&edge_i.txt_file.replace(".txt", ".done"));
                write_done_file(&edge_j.txt_file.replace(".txt", ".done"));*/
            }
            _ => {}
        }
    }
}
