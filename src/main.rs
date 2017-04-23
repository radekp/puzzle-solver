extern crate sdl2;
extern crate image;

use std::fs;
use std::cmp;
use std::env;
use std::thread;
use std::fs::File;
use std::path::Path;
use std::ffi::OsStr;
use std::str::FromStr;
use std::error::Error;
use std::io::prelude::*;
use std::time::Duration;
use std::fs::OpenOptions;
use std::collections::HashMap;

use sdl2::pixels;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::event::Event;
use sdl2::pixels::Color;
use sdl2::keyboard::Keycode;
use sdl2::image::LoadTexture;
use sdl2::render::TextureQuery;
use sdl2::render::Renderer;
use sdl2::render::Texture;
use sdl2::gfx::primitives::DrawRenderer;

use image::Pixel;
use image::GenericImage;

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
    edge_no: usize, // e.g. 103 is 10.3.txt
    edge_index: usize, // index to edges vector
    max_x: usize,
    max_y: usize,
    diff_to: Vec<usize>, // distance sum to edge at given index (in edges vector)
    best_diff: Vec<(usize, usize)>, // top 10 (edge_index, diff)
    solved_index: usize, // for solved edge_index to the other, for unsolved usize::max_value
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
/*fn near_iter_begin(cx: isize, cy: isize, start_a: isize) -> (isize, isize, isize) {
    return (cx - start_a, cy - start_a, start_a);
}

// Return next point in spiral
fn near_iter_next(cx: isize,
                  cy: isize,
                  prev_x: isize,
                  prev_y: isize,
                  prev_a: isize)
                  -> (isize, isize, isize) {

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

fn get_points(pixels: &Vec<u8>, sqr: usize, bounds: URect, red_mask: u8) -> Vec<(usize, usize)> {
    let mut res = vec![];
    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            if pixels[3 * (sqr * y + x)] & red_mask != 0 {
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
    detect_jags(&mut pixels, sqr, bounds, sqr / 48, sqr / 6, sqr / 6);

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
             -> Vec<(usize, usize)> {

    // Split border in top and bot points into 2 parts
    for i in 0..10 {
        pixels[3 * (sqr * (top_y - i) + top_x - i)] &= !RED_MASK_BORDER;
        pixels[3 * (sqr * (top_y + i) + top_x + i)] &= !RED_MASK_BORDER;
        pixels[3 * (sqr * (bot_y + i) + bot_x - i)] &= !RED_MASK_BORDER;
        pixels[3 * (sqr * (bot_y - i) + bot_x + i)] &= !RED_MASK_BORDER;

        // Make it 2points thin so that flood fill cant go through diagonal
        pixels[3 * (sqr * (top_y - i) + top_x - i + 1)] &= !RED_MASK_BORDER;
        pixels[3 * (sqr * (top_y + i) + top_x + i + 1)] &= !RED_MASK_BORDER;
        pixels[3 * (sqr * (bot_y + i) + bot_x - i + 1)] &= !RED_MASK_BORDER;
        pixels[3 * (sqr * (bot_y - i) + bot_x + i + 1)] &= !RED_MASK_BORDER;

        pixels[3 * (sqr * (top_y - i) + top_x - i) + 1] = (255 - 25 * i) as u8;
        pixels[3 * (sqr * (top_y + i) + top_x + i) + 1] = (255 - 25 * i) as u8;
        pixels[3 * (sqr * (bot_y + i) + bot_x - i) + 1] = (255 - 25 * i) as u8;
        pixels[3 * (sqr * (bot_y - i) + bot_x + i) + 1] = (255 - 25 * i) as u8;

        pixels[3 * (sqr * (top_y - i) + top_x - i + 1) + 1] = (255 - 25 * i) as u8;
        pixels[3 * (sqr * (top_y + i) + top_x + i + 1) + 1] = (255 - 25 * i) as u8;
        pixels[3 * (sqr * (bot_y + i) + bot_x - i + 1) + 1] = (255 - 25 * i) as u8;
        pixels[3 * (sqr * (bot_y - i) + bot_x + i + 1) + 1] = (255 - 25 * i) as u8;
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

    let mut edge = get_points(pixels, sqr, bounds, RED_MASK_FLOOD_FILLED);

    draw_coords(pixels, sqr, &edge, 0, 0, 0, 0, 255);

    // Sort edge by y and then by x, so that max_x,max_y is last so that
    // compare can be fast
    edge.sort_by(|a, b| (a.1 * sqr + a.0).cmp(&(b.1 * sqr + b.0)));

    edge
}

#[derive(Debug)]
enum UserAction {
    Rotate,
    Autorotate,
    Quit,
    Solve,
    Compute,
    NoAction,
    Number(usize),
    Delete,
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

    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut dst_rect = Rect::new(0, 0, sqr as u32, sqr as u32);

    let mut num = 0;

    loop {
        renderer.clear();
        renderer.copy(&res_texture, None, Some(dst_rect)).unwrap();
        renderer.present();
        for event in event_pump.poll_iter() {
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
                    return UserAction::Autorotate;
                }
                Event::KeyDown { keycode: Some(Keycode::S), .. } => {
                    return UserAction::Solve;
                }
                Event::KeyDown { keycode: Some(Keycode::D), .. } => {
                    return UserAction::Delete;
                }
                Event::KeyDown { keycode: Some(Keycode::C), .. } => {
                    return UserAction::Compute;
                }
                Event::KeyDown { keycode: Some(Keycode::Num0), .. } => {
                    state.autorotate = false;
                    num = num * 10;
                    println!("num={}", num);
                }
                Event::KeyDown { keycode: Some(Keycode::Num1), .. } => {
                    state.autorotate = false;
                    num = num * 10 + 1;
                    println!("num={}", num);
                }
                Event::KeyDown { keycode: Some(Keycode::Num2), .. } => {
                    state.autorotate = false;
                    num = num * 10 + 2;
                    println!("num={}", num);
                }
                Event::KeyDown { keycode: Some(Keycode::Num3), .. } => {
                    state.autorotate = false;
                    num = num * 10 + 3;
                    println!("num={}", num);
                }
                Event::KeyDown { keycode: Some(Keycode::Num4), .. } => {
                    state.autorotate = false;
                    num = num * 10 + 4;
                    println!("num={}", num);
                }
                Event::KeyDown { keycode: Some(Keycode::Num5), .. } => {
                    state.autorotate = false;
                    num = num * 10 + 5;
                    println!("num={}", num);
                }
                Event::KeyDown { keycode: Some(Keycode::Num6), .. } => {
                    state.autorotate = false;
                    num = num * 10 + 6;
                    println!("num={}", num);
                }
                Event::KeyDown { keycode: Some(Keycode::Num7), .. } => {
                    state.autorotate = false;
                    num = num * 10 + 7;
                    println!("num={}", num);
                }
                Event::KeyDown { keycode: Some(Keycode::Num8), .. } => {
                    state.autorotate = false;
                    num = num * 10 + 8;
                    println!("num={}", num);
                }
                Event::KeyDown { keycode: Some(Keycode::Num9), .. } => {
                    state.autorotate = false;
                    num = num * 10 + 9;
                    println!("num={}", num);
                }
                Event::KeyDown { keycode: Some(Keycode::Return), .. } => {
                    println!("return num={}", num);
                    return UserAction::Number(num);
                }
                Event::Quit { .. } |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => return UserAction::Quit,
                _ => {}
            }
        }
        if state.autorotate {
            return UserAction::NoAction;
        }
        thread::sleep(Duration::from_millis(100))
    }
}

fn save_points(points: &Vec<(usize, usize)>, img_file: &str, filename: &str) {

    // Find min
    let mut min_x = usize::max_value();
    let mut min_y = usize::max_value();
    for p in points.iter() {
        min_x = cmp::min(p.0, min_x);
        min_y = cmp::min(p.1, min_y);
    }

    let mut content: String = "".to_string();
    for p in points.iter() {
        if content.len() > 0 {
            content += "\n";
        }
        content = content + &format!("{},{}", p.0 - min_x, p.1 - min_y);
    }


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

        let mut r = -25f64;
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
            if corner_delta <= best_corner_delta {
                best_corner_delta = corner_delta;
                best_corner_angle = angle;
            }

            match display_pixels(&pixels, sqr, sdl_context, &mut renderer, display_state) {
                UserAction::Quit => break 'rotating,
                UserAction::Compute => {
                    r -= 1f64;
                    best_corner_delta = usize::max_value();
                    continue 'rotating;
                }
                _ => {}
            }

            if corner_delta > 10 {
                r += 1f64;
            } else if corner_delta > 2 {
                r += 0.5f64;
            } else {
                r += 0.02f64;
            }
            if r > 25f64 {
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

        // Save all border points to file
        if side == 0 {
            let border = get_points(&pixels, sqr, bounds, RED_MASK_BORDER);
            save_points(&border, img_file, &format!("{}.txt", png_no));
        }

        // Save left edge coordinates to file
        let edge = find_edge(&mut pixels, sqr, bounds, top_x, top_y, bot_x, bot_y);
        save_points(&edge, img_file, &format!("{}.{}.txt", png_no, side));

        // Make .done file so that we can detect processed pngs
        if side == 3 {
            write_done_file(img_file);
        }

        display_pixels(&pixels, sqr, sdl_context, &mut renderer, display_state);
    }
}

fn process_jpg(jpg_file: &str, jpg_no: usize, sdl_context: &sdl2::Sdl) {

    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window(jpg_file, WND_WIDTH as u32, WND_HEIGHT as u32)
        .position(200, 0)
        .opengl()
        .build()
        .unwrap();

    let mut renderer = window.renderer().build().unwrap();

    let texture = renderer.load_texture(jpg_file).unwrap();

    let TextureQuery { width, height, .. } = texture.query();

    let mut event_pump = sdl_context.event_pump().unwrap();

    let dst_rect = Rect::new(0, 0, WND_WIDTH as u32, WND_HEIGHT as u32);

    let mut down_x = -1;
    let mut down_y = -1;
    let mut png_no = jpg_no;

    // Use the open function to load an image from a Path.
    // ```open``` returns a dynamic image.
    let img = image::open(&Path::new(jpg_file)).unwrap();

    // The dimensions method returns the images width and height
    println!("dimensions {:?}", img.dimensions());

    // The color method returns the image's ColorType
    println!("{:?}", img.color());

    loop {
        for event in event_pump.poll_iter() {
            renderer.clear();
            renderer.copy(&texture, None, Some(dst_rect)).unwrap();
            renderer.present();
            match event {

                Event::MouseButtonDown { x, y, .. } => {
                    down_x = x;
                    down_y = y;
                }
                Event::MouseButtonUp { x, y, .. } => {

                    let png_file = format!("{}.png", png_no);

                    let left = (down_x as u32 * width) / WND_WIDTH as u32;
                    let top = (down_y as u32 * height) / WND_HEIGHT as u32;
                    let width = (x as u32 * width) / WND_WIDTH as u32 - left;
                    let height = (y as u32 * height) / WND_HEIGHT as u32 - top;

                    println!("saving {} {},{} {}x{}", png_file, left, top, width, height);

                    down_x = -1;

                    let mut imgbuf = image::ImageBuffer::new(width, height);

                    // Iterate over the coordiantes and pixels of the image
                    for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
                        let pix = img.get_pixel(left + x, top + y).to_luma();
                        if pix.data[0] > 78 {
                            *pixel = image::Luma([255u8]);
                        } else {
                            *pixel = image::Luma([0u8]);
                        }
                    }

                    let ref mut fout = File::create(&Path::new(&png_file)).unwrap();
                    // Write the contents of this image to the Writer in PNG format.
                    let _ = image::ImageLuma8(imgbuf).save(fout, image::PNG);

                    png_no += 1;
                }

                Event::MouseMotion { x, y, .. } => {
                    let color = pixels::Color::RGB(x as u8, y as u8, 255);
                    if down_x < 0 {
                        let _ = renderer.line(x as i16, 0, x as i16, WND_HEIGHT as i16, color);
                        let _ = renderer.line(0, y as i16, WND_WIDTH as i16, y as i16, color);
                    } else {
                        let _ =
                        renderer.rectangle(down_x as i16, down_y as i16, x as i16, y as i16, color);
                    }
                    renderer.present();
                }

                Event::KeyDown { keycode: Some(Keycode::Left), .. } => {
                    png_no -= 1;
                    println!("png_no={}", png_no);
                }
                Event::KeyDown { keycode: Some(Keycode::Right), .. } => {
                    png_no += 1;
                    println!("png_no={}", png_no);
                }
                Event::KeyDown { keycode: Some(Keycode::Up), .. } => {
                    png_no = png_no - png_no % 10 + 10;
                    println!("png_no={}", png_no);
                }
                Event::KeyDown { keycode: Some(Keycode::Down), .. } => {
                    png_no = png_no - png_no % 10 - 10;
                    println!("png_no={}", png_no);
                }
                Event::Quit { .. } |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    write_done_file(jpg_file);
                    return;
                }
                _ => {}
            }
        }
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
        if v.len() != 2 {
            continue;
        }
        coords.push((usize::from_str(&v[0].replace(".", "")).unwrap(),
                     usize::from_str(&v[1].replace(".", "")).unwrap()));
    }

    return coords;
}

fn max_xy(coords: &Vec<(usize, usize)>) -> (usize, usize) {

    let mut max_x = 0;
    let mut max_y = 0;
    for p in coords {
        max_x = cmp::max(p.0, max_x);
        max_y = cmp::max(p.1, max_y);
    }
    (max_x, max_y)
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

fn draw_edge(pixels: &mut Vec<u8>,
             edges: &Vec<EdgeInfo>,
             e_index: usize,
             flip: bool,
             sqr: usize,
             left: usize,
             top: usize,
             color_r: u8,
             color_g: u8,
             color_b: u8) {

    let ref edge_e = edges[e_index];
    let (r, g, b) = if edge_e.solved_index == usize::max_value() {
        (color_r, color_g, color_b)
    } else {
        (255, 255, 255)
    };

    if flip {
        draw_coords(pixels,
                    sqr,
                    &flip_coords(&edge_e.points),
                    left,
                    top,
                    r,
                    g,
                    b);
    } else {
        draw_coords(pixels, sqr, &edge_e.points, left, top, r, g, b);
    }
}

fn flip_coords(coords: &Vec<(usize, usize)>) -> Vec<(usize, usize)> {

    let mut max_x = 0;
    let mut max_y = 0;

    for p in coords {
        max_x = cmp::max(p.0, max_x);
        max_y = cmp::max(p.1, max_y);
    }

    let mut res = Vec::with_capacity(coords.len());
    for p in coords {
        res.push((max_x - p.0, max_y - p.1));
    }
    res.reverse();
    return res;
}

fn rotate_piece(points: &Vec<(usize, usize)>, side: usize) -> Vec<(usize, usize)> {

    let max = max_xy(&points);
    let mut res = Vec::with_capacity(points.len());

    if side == 0 {
        for p in points.iter() {
            res.push((p.0 / 2, p.1 / 2));
        }
    } else if side == 1 {
        for p in points.iter() {
            res.push((p.1 / 2, (max.0 - p.0) / 2));
        }
    } else if side == 2 {
        for p in points.iter() {
            res.push(((max.0 - p.0) / 2, (max.1 - p.1) / 2));
        }
    } else {
        for p in points.iter() {
            res.push(((max.1 - p.1) / 2, p.0 / 2));
        }
    }
    res
}

// Used to draw piece with solved edge with white
fn piece_col(edges: &Vec<EdgeInfo>, piece_no: usize, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    for edge in edges {
        if edge.solved_index != usize::max_value() && edge.edge_no >> 2 == piece_no {
            return (255, 255, 255);
        }
    }
    (r, g, b)
}

fn compare_edge_with_others(edges: &mut Vec<EdgeInfo>,
                            e_index: usize,
                            max_width: usize,
                            max_height: usize) {

    if edges[e_index].diff_to.len() > 0 {
        return;
    }
    let edges_len = edges.len();
    edges[e_index].diff_to = vec![usize::max_value();edges_len];

    // For each x,y there is distance to nearest point on edge e
    let mut distances = vec![usize::max_value();max_width*max_height];

    let e_max_x = edges[e_index].max_x;
    let e_max_y = edges[e_index].max_y;

    for f_index in 0..edges_len {
        if f_index == e_index {
            continue;
        }
        let mut diff = 0;
        for f in edges[f_index].points.iter() {
            let offset = max_width * f.1 + f.0;
            let mut best_dst = distances[offset]; // precomputed distance

            // Compute best distance to edge from given x,y (point f) on first hit
            if best_dst == usize::max_value() {
                for point_e in edges[e_index].points.iter() {
                    // One point must be flipped
                    let e = (e_max_x - point_e.0, e_max_y - point_e.1);
                    let dx = (e.0 as isize) - (f.0 as isize);
                    let dy = (e.1 as isize) - (f.1 as isize);
                    let dst = (dx * dx + dy * dy) as usize;
                    if dst < best_dst {
                        best_dst = dst;
                    }
                }
                distances[offset] = best_dst;
            }
            diff += best_dst;
        }
        edges[e_index].diff_to[f_index] = diff;
    }
}

fn compare_edges(edges: &Vec<EdgeInfo>, index_b: usize, index_a: usize) -> usize {

    // Use diff_to if computed, otherwise get max_x and max_y from b
    let ref edge_b = edges[index_b];
    if edge_b.diff_to.len() > 0 {
        return edge_b.diff_to[index_a];
    }

    let mut diff = 0;
    for a in edges[index_a].points.iter() {
        let mut best_dst = usize::max_value();
        for point_b in edges[index_b].points.iter() {
            // One point must be flipped
            let b = (edge_b.max_x - point_b.0, edge_b.max_y - point_b.1);
            let dx = (b.0 as isize) - (a.0 as isize);
            let dy = (b.1 as isize) - (a.1 as isize);
            let dst = (dx * dx + dy * dy) as usize;
            if dst < best_dst {
                best_dst = dst;
            }
        }
        diff += best_dst;
    }

    return diff;
}

// Compute egge.best_diff vector
fn compute_best_diff(i: usize,
                     mut edges: &mut Vec<EdgeInfo>,
                     last_best_index: usize,
                     max_width: usize,
                     max_height: usize) {

    // Already computed?
    if edges[i].best_diff.len() > last_best_index {
        return;
    }

    // If solved make make (solved edge, zero diff) vector
    let num_best = last_best_index + 1;
    if edges[i].solved_index != usize::max_value() {
        edges[i].best_diff = vec![(edges[i].solved_index, 0); num_best];
        return;
    }

    let edges_len = edges.len();

    // Compare self with all other edges
    compare_edge_with_others(&mut edges, i, max_width, max_height);

    let mut diffs = Vec::with_capacity(edges_len);
    for j in 0..edges_len {
        diffs.push((edges[i].diff_to[j], j));
    }
    diffs.sort_by(|a, b| (a.0).cmp(&b.0));

    // Init best_diff with 10 values - index must be != me (i+1) % edges_len works
    let mut best_diff = vec![((i + 1) % edges_len, usize::max_value()); num_best];

    // We will take diff for each i->j compare
    for (diff_ij, j) in diffs {

        if i == j {
            continue; // dont compare with self
        }

        if diff_ij > best_diff[last_best_index].1 {
            continue; // even one way compare is worse then last one...
        }

        // Add diff for j->i direction
        let diff = diff_ij + compare_edges(edges, j, i);

        for k in 0..num_best {
            // (index, diff) of k.th best
            let mut b = best_diff[k];
            if diff > b.1 {
                continue;
            }

            /*print!("i={} j={} b.1={} diff_ij={} diff={} ", i, j, b.1, diff_ij, diff);
            for x in best_diff.iter() {
                print!("{} ", x.1);
            }
            println!("");*/

            best_diff[k] = (j, diff); // replace best
            let mut kk = k + 1;
            while kk < num_best {
                // places the prev best after it
                let tmp = best_diff[kk];
                best_diff[kk] = b;
                b = tmp;
                kk += 1;
            }
            break; // add to vector next j
        }
    }
    edges[i].best_diff = best_diff;

    /*let i_no = edges[i].edge_no;
    print!("best diffs for {}.{}: ", i_no >> 2, i_no & 3);
    for b in edges[i].best_diff.iter() {
        let b_no = edges[b.0].edge_no;
        print!("({}.{},{})", b_no >> 2, b_no & 3, b.1);
    }
    println!("");*/
}

// Return nth best (edge_index, edge_no, diff)
fn get_best_diff(e_index: usize,
                 mut edges: &mut Vec<EdgeInfo>,
                 n: usize,
                 max_width: usize,
                 max_height: usize)
                 -> (usize, usize, usize) {
    // For solved return
    let solved_index = edges[e_index].solved_index;
    if solved_index != usize::max_value() {
        return (solved_index, edges[solved_index].edge_no, 0);
    }

    // We need loop n.. lopp, because some best_diff values are skipped
    let e_no = edges[e_index].edge_no;
    for m in n..edges.len() {

        // Make sure we have best_diff computed for m
        compute_best_diff(e_index, &mut edges, m + 1, max_width, max_height);
        let (a, diff_a) = edges[e_index].best_diff[m];
        let a_no = edges[a].edge_no;

        // Skip solved edges
        if edges[a].solved_index != usize::max_value() {
            continue;
        }

        // Skip edges of the same piece
        if e_no == a_no {
            continue;
        }

        return (a, a_no, diff_a);
    }
    panic!("get_best_diff hasnt found anything"); // when all is solved?
}

// Return next side of the piece
fn side_plus(edge_no: usize) -> usize {
    return (edge_no & !3) | ((edge_no + 1) & 3);
}

// Return prev side of the piece
fn side_minus(edge_no: usize) -> usize {
    return (edge_no & !3) | ((edge_no + 3) & 3);
}

// Make file processed
fn write_done_file(path: &str) {
    let done_str = path.to_string() + ".done";
    println!("writting done file {}", done_str);
    let done_path = Path::new(&done_str);
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
    let done_str = path.to_string() + ".done";
    let done_path = Path::new(&done_str);
    if !done_path.exists() {
        return false;
    }
    println!("skipping {} because {} exists", path, done_path.display());

    return true;
}

fn main() {
    let sdl_context = sdl2::init().unwrap();

    let mut display_state = DisplayPixelState { autorotate: false };

    // Process all .jpg files
    let entries = fs::read_dir("./jpg").unwrap();
    for entry in entries {
        //println!("Name: {}", path.unwrap().path().into_os_string().into_string());

        let path = entry.unwrap().path();
        match path.extension().and_then(OsStr::to_str) {
            Some("jpg") => {
                let jpg_no: usize = path.file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .parse()
                    .unwrap();
                let path_str = path.into_os_string().into_string().unwrap();
                if is_done(&path_str) {
                    continue;
                }
                process_jpg(&path_str, jpg_no, &sdl_context);
            }
            _ => {}
        }
    }

    // Process all .png files - this will write 4 txt files for each edge
    let entries = fs::read_dir("./data").unwrap();
    for entry in entries {
        //println!("Name: {}", path.unwrap().path().into_os_string().into_string());

        let path = entry.unwrap().path();
        match path.extension().and_then(OsStr::to_str) {
            Some("png") => {
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
            _ => {}
        }
    }
    //process_png("9.png", &sdl_context);

    // Read txt files with edges
    let mut edges = vec![];
    let mut pieces = HashMap::new();
    let entries = fs::read_dir("./data").unwrap();
    for entry in entries {

        let path = entry.unwrap().path();
        if path.extension().unwrap() != "txt" {
            continue;
        }

        let (edge_no, piece_no) = {

            let file_stem = path.file_stem()
                .unwrap()
                .to_str()
                .unwrap();

            let filename_nums: usize = file_stem.replace(".", "").parse().unwrap();

            if file_stem.contains(".") {
                // edge no: 12.3.txt -> 123 -> 4 * 12 + 3
                (4 * (filename_nums / 10) + (filename_nums % 10), usize::max_value())
            } else {
                (usize::max_value(), filename_nums) // piece_no
            }
        };

        let path_str = path.into_os_string().into_string().unwrap();
        let points = read_txt(&path_str);

        // If it's pieces, just read points
        if piece_no != usize::max_value() {
            pieces.insert(piece_no, points);
            continue;
        }

        // It's edge. Compute height and add EdgeInfo
        let mut max_x = 0;
        let mut max_y = 0;
        for p in points.iter() {
            max_x = cmp::max(max_x, p.0);
            max_y = cmp::max(max_y, p.1);
        }

        let edge_info = EdgeInfo {
            points: points,
            edge_no: edge_no,
            max_x: max_x,
            max_y: max_y,
            diff_to: vec![],
            best_diff: vec![],
            edge_index: usize::max_value(),
            solved_index: usize::max_value(),
        };
        edges.push(edge_info);
    }

    let edges_len = edges.len();

    // Max x and y in all edges, make diff_to vector
    let mut max_x = 0;
    let mut max_y = 0;
    let mut max_x_edge_no = 0;
    let mut max_y_edge_no = 0;
    for edge in edges.iter_mut() {
        if edge.max_x > max_x {
            max_x = edge.max_x;
            max_x_edge_no = edge.edge_no;
        }
        if edge.max_y > max_y {
            max_y = edge.max_y;
            max_y_edge_no = edge.edge_no;
        }
    }
    let max_width = max_x + 1;
    let max_height = max_y + 1;

    println!("MAX x: {}.{}={} y: {}.{}={}",
             max_x_edge_no >> 2,
             max_x_edge_no & 3,
             max_x,
             max_y_edge_no >> 2,
             max_y_edge_no & 3,
             max_y);

    // SDL window - make it modulo 4 to play well with texture pitch
    let sqr = 3 * cmp::max(max_width, max_height) + 5 & !3usize;

    let mut pixels: Vec<u8> = vec![0;3*sqr*sqr];
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("puzzle solver", sqr as u32, sqr as u32)
        .opengl()
        .build()
        .unwrap();

    let mut renderer = window.renderer().build().unwrap();

    // Hashmap to get index by edge_no
    let mut edge_nums = HashMap::new();
    for i in 0..edges_len {
        let ref mut edge_i = edges[i];
        let i_no = edge_i.edge_no;
        //println!("edge={}.{}", i_no >> 2, i_no & 3);
        edge_nums.insert(i_no, i);
        edge_i.edge_index = i;
    }

    // Solved edges
    let mut pref_solved = vec![];
    for p in read_txt("solved_edges.txt") {
        let i_no = 4 * (p.0 / 10) + (p.0 % 10); // edge no: 12.3 -> 123 -> 4 * 12 + 3
        let j_no = 4 * (p.1 / 10) + (p.1 % 10);
        print!("solved edge {:>4}.{}->{:>4}.{}",
               i_no >> 2,
               i_no & 3,
               j_no >> 2,
               j_no & 3);
        let i_index = *edge_nums.get(&i_no).unwrap();
        let j_index = *edge_nums.get(&j_no).unwrap();
        edges[i_index].solved_index = j_index;
        edges[j_index].solved_index = i_index;

        let diff_ij = compare_edges(&edges, i_index, j_index);
        let diff_ji = compare_edges(&edges, j_index, i_index);

        pref_solved.insert(0, i_index);
        pref_solved.insert(0, j_index);

        println!(", diff {:>12}+{:<12}={:>12}",
                 diff_ij,
                 diff_ji,
                 diff_ij + diff_ji);
    }

    // Prefer pieces from command line
    let mut pref_cmd_solved = vec![];
    let mut pref_cmd_unsolved = vec![];
    for arg in env::args().skip(1) {
        let argv: usize = arg.parse().unwrap();
        for i in 0..edges_len {
            let png_no = edges[i].edge_no >> 2;
            if png_no != argv {
                continue;
            }
            if edges[i].solved_index == usize::max_value() {
                pref_cmd_unsolved.insert(0, i);
            } else {
                pref_cmd_solved.insert(0, i);
            }
        }
    }

    // Prefered cmd line, then solved edges
    let mut pref_indices = vec![];
    pref_indices.append(&mut pref_cmd_solved);
    pref_indices.append(&mut pref_cmd_unsolved);
    pref_indices.append(&mut pref_solved);

    println!("Compared edges:");
    println!("");
    println!("   1st     2nd     3rd   4th          score");

    let mut pref_new = vec![];

    let mut combi_shift = 0;

    loop {
        for pref in pref_new.iter() {
            pref_indices.insert(0, *pref);
        }

        // Do 4-edges compare (edges from pieces A,B,C,D)
        //     C  ->  D
        //     ^      |
        //     |      v
        //     B  <-  A
        'pref_indices_loop: for a_item in pref_indices.iter() {
            let a = *a_item;
            let a_no = edges[a].edge_no;

            // Loop to compare combination of best edges, e.g. 1stJ..1stP, 1stJ..2ndM, 2ndJ..2ndM
            let mut best_final_score = usize::max_value();
            let mut combi_counter = 0;
            let mut best_combi_counter = 0;

            // Parameter for edge matching combinations
            let combi_one_edge = 1 << combi_shift; // number of combinations for one edge
            let combi_mask = combi_one_edge - 1;
            let combi_all = combi_one_edge * combi_one_edge * combi_one_edge;

            'combi_loop: loop {

                // Last round displays the best result
                let combi_val = if combi_counter <= combi_all - 1 {
                    combi_counter
                } else if combi_counter == combi_all {
                    println!("======= BEST MATCH {:>2} ========", best_combi_counter);
                    display_state.autorotate = false;
                    best_combi_counter
                } else {
                    break 'combi_loop;
                };

                let combi = (combi_val & combi_mask,
                             (combi_val >> combi_shift) & combi_mask,
                             (combi_val >> (2 * combi_shift)) & combi_mask);
                if combi_counter != combi_all {
                    println!("------------------------------              combi {}=>{}.{}.{}",
                             combi_counter,
                             combi.0,
                             combi.1,
                             combi.2);
                }

                combi_counter += 1;

                //     B  <-  A
                let (b, b_no, diff_b) =
                    get_best_diff(a, &mut edges, combi_mask, max_width, max_height);

                println!("{:>4}.{}->{:>4}.{}                 {:>12}",
                         a_no >> 2,
                         a_no & 3,
                         b_no >> 2,
                         b_no & 3,
                         diff_b);

                //     C
                //     ^
                //     |
                //     B  <-  A
                let b_plus_no = side_plus(b_no);
                let b_plus = *edge_nums.get(&b_plus_no).unwrap();
                let (c, c_no, diff_c) =
                    get_best_diff(b_plus, &mut edges, combi_mask, max_width, max_height);

                println!("        {:>4}.{}->{:>4}.{}         {:>12}",
                         b_plus_no >> 2,
                         b_plus_no & 3,
                         c_no >> 2,
                         c_no & 3,
                         diff_c);

                //     C  ->  D
                //     ^
                //     |
                //     B  <-  A
                let c_plus_no = side_plus(c_no);
                let c_plus = *edge_nums.get(&c_plus_no).unwrap();
                let (d, d_no, diff_d) =
                    get_best_diff(c_plus, &mut edges, combi_mask, max_width, max_height);

                println!("                {:>4}.{}->{:>4}.{} {:>12}",
                         c_plus_no >> 2,
                         c_plus_no & 3,
                         d_no >> 2,
                         d_no & 3,
                         diff_d);

                // Now check A->D - must be small if pieces fit
                //
                //     C  ->  D
                //     ^      ^
                //     |      |
                //     B  <-  A
                //
                // The last edge can be marked as solved and thus not loaded
                let d_plus_no = side_plus(d_no);
                let d_plus = *edge_nums.get(&d_plus_no).unwrap();

                let a_minus_no = side_minus(a_no);
                let a_minus = *edge_nums.get(&a_minus_no).unwrap();

                let mut diff_a_minus = compare_edges(&mut edges, a_minus, d_plus) +
                                       compare_edges(&mut edges, d_plus, a_minus);


                // Check if it's not the same edge
                if d_plus == a_minus {
                    println!("SKIP d_plus and a_minus is same edge");
                    diff_a_minus += 100000000;
                }

                // Check if solved d->a match
                let d_plus_solved_index = edges[d_plus].solved_index;
                if d_plus_solved_index != usize::max_value() {
                    diff_a_minus = if d_plus_solved_index == a_minus {
                        0
                    } else {
                        println!("SKIP {}.{} is already solved to {}.{} and does not match {}.{}",
                                 d_plus_no >> 2,
                                 d_plus_no & 3,
                                 edges[d_plus_solved_index].edge_no >> 2,
                                 edges[d_plus_solved_index].edge_no & 3,
                                 a_minus_no >> 2,
                                 a_minus_no & 3);
                        diff_a_minus + 100000000
                    }
                }
                let a_minus_solved_index = edges[a_minus].solved_index;
                if a_minus_solved_index != usize::max_value() {
                    diff_a_minus = if a_minus_solved_index == d_plus {
                        if diff_a_minus == 0 {
                            0
                        } else {
                            panic!("a_minus solved to d_plus but d_plus solved to other");
                        }
                    } else {
                        println!("SKIP {}.{} is already solved to {}.{} and does not match {}.{}",
                                 a_minus_no >> 2,
                                 a_minus_no & 3,
                                 edges[a_minus_solved_index].edge_no >> 2,
                                 edges[a_minus_solved_index].edge_no & 3,
                                 d_plus_no >> 2,
                                 d_plus_no & 3);
                        diff_a_minus + 100000000
                    }
                }

                let mut skip_draw = display_state.autorotate;

                let final_score = diff_b + diff_c + diff_d + diff_a_minus;

                println!("{:>4}.{}<-                {:>4}.{} {:>12} FINAL SCORE={}",
                         a_minus_no >> 2,
                         a_minus_no & 3,
                         d_plus_no >> 2,
                         d_plus_no & 3,
                         diff_a_minus,
                         final_score);

                // Remeber best 4-edge diff that will be displayed after all cominations computed
                if final_score < best_final_score {
                    best_final_score = final_score;
                    best_combi_counter = combi_val;
                    skip_draw = false; // always draw the best matching
                }

                if skip_draw {
                    continue;
                }

                // Display comapred edges
                for p in pixels.iter_mut() {
                    *p = 0;
                }

                draw_edge(&mut pixels, &edges, a, false, sqr, 0, 0, 255, 0, 0);
                draw_edge(&mut pixels, &edges, b, true, sqr, 0, 0, 0, 255, 0);

                draw_edge(&mut pixels, &edges, b_plus, false, sqr, 100, 0, 255, 0, 0);
                draw_edge(&mut pixels, &edges, c, true, sqr, 100, 0, 0, 255, 0);

                draw_edge(&mut pixels, &edges, c_plus, false, sqr, 200, 0, 255, 0, 0);
                draw_edge(&mut pixels, &edges, d, true, sqr, 200, 0, 0, 255, 0);

                draw_edge(&mut pixels, &edges, d_plus, false, sqr, 300, 0, 255, 0, 0);
                draw_edge(&mut pixels, &edges, a_minus, true, sqr, 300, 0, 0, 255, 0);

                let piece_a = rotate_piece(pieces.get(&(a_no >> 2)).unwrap(), 0);
                let piece_b = rotate_piece(pieces.get(&(b_no >> 2)).unwrap(), 0);
                let piece_c = rotate_piece(pieces.get(&(c_no >> 2)).unwrap(), 0);
                let piece_d = rotate_piece(pieces.get(&(d_no >> 2)).unwrap(), 0);
                let max_a = max_xy(&piece_a);

                let col_a = piece_col(&edges, a_no >> 2, 255, 0, 0);
                let col_b = piece_col(&edges, b_no >> 2, 0, 255, 0);
                let col_c = piece_col(&edges, c_no >> 2, 0, 0, 255);
                let col_d = piece_col(&edges, d_no >> 2, 255, 255, 0);

                draw_coords(&mut pixels,
                            sqr,
                            &piece_a,
                            max_a.0,
                            max_height + max_a.1,
                            col_a.0,
                            col_a.1,
                            col_a.2);
                draw_coords(&mut pixels,
                            sqr,
                            &piece_b,
                            0,
                            max_height + max_a.1,
                            col_b.0,
                            col_b.1,
                            col_b.2);
                draw_coords(&mut pixels,
                            sqr,
                            &piece_c,
                            0,
                            max_height,
                            col_c.0,
                            col_c.1,
                            col_c.2);
                draw_coords(&mut pixels,
                            sqr,
                            &piece_d,
                            max_a.0,
                            max_height,
                            col_d.0,
                            col_d.1,
                            col_d.2);

                // Go on if all 4edges solved
                if final_score == 0 {
                    break 'combi_loop;
                }

                // Content for solved_edges.txt
                let solved_str = {
                    if display_state.autorotate {
                        "".to_string()
                    } else {
                        let solved_tmp = format!("{}.{},{}.{}\n{}.{},{}.{}\n{}.{},{}.{}\n{}.{},\
                                                  {}.{}\n",
                                                 a_no >> 2,
                                                 a_no & 3,
                                                 b_no >> 2,
                                                 b_no & 3,
                                                 b_plus_no >> 2,
                                                 b_plus_no & 3,
                                                 c_no >> 2,
                                                 c_no & 3,
                                                 c_plus_no >> 2,
                                                 c_plus_no & 3,
                                                 d_no >> 2,
                                                 d_no & 3,
                                                 a_minus_no >> 2,
                                                 a_minus_no & 3,
                                                 d_plus_no >> 2,
                                                 d_plus_no & 3);
                        println!("\n{}", solved_tmp);
                        solved_tmp
                    }
                };

                // Display result and use time for user key to compute diffs
                'display_and_precompute: loop {

                    // autorotate=true will not wait for key
                    let autorotate_save = display_state.autorotate;
                    display_state.autorotate = true;
                    let display_res = display_pixels(&pixels,
                                                     sqr,
                                                     &sdl_context,
                                                     &mut renderer,
                                                     &mut display_state);


                    if autorotate_save {
                        break 'display_and_precompute;
                    }
                    display_state.autorotate = !display_state.autorotate;

                    match display_res {
                        UserAction::Solve => {
                            let mut file = OpenOptions::new()
                                .write(true)
                                .append(true)
                                .open("solved_edges.txt")
                                .unwrap();

                            if let Err(e) = file.write_all(solved_str.as_bytes()) {
                                println!("{}", e);
                            } else {
                                println!("written to solved_edges.txt");
                            }
                            edges[a].solved_index = b;
                            edges[b].solved_index = a;

                            edges[b_plus].solved_index = c;
                            edges[c].solved_index = b_plus;

                            edges[c_plus].solved_index = d;
                            edges[d].solved_index = c_plus;

                            edges[d_plus].solved_index = a_minus;
                            edges[a_minus].solved_index = d_plus;
                            break;
                        }
                        UserAction::Delete => {
                            println!("{:?}",
                                     fs::remove_file(format!("data/{}.png.done", a_no >> 2)));
                            println!("{:?}",
                                     fs::remove_file(format!("data/{}.png.done", b_no >> 2)));
                            /*println!("{:?}",
                                     fs::remove_file(format!("data/{}.png.done", c_no >> 2)));
                            println!("{:?}",
                                     fs::remove_file(format!("data/{}.png.done", d_no >> 2)));*/
                            break;
                        }
                        UserAction::Number(num) => {
                            pref_new.clear();
                            for i in 0..4 {
                                let edge_no = 4 * num + i;
                                let idx = edge_nums.get(&edge_no);
                                if idx.is_some() {
                                    pref_new.push(*idx.unwrap());
                                } else {
                                    println!("{} not found", num);
                                }
                            }
                            break 'pref_indices_loop;
                        }
                        UserAction::Compute => {
                            combi_shift = (combi_shift + 1) % 4;
                            println!("combi_shift={}", combi_shift);
                            pref_new.clear();
                            pref_new.push(a);
                            display_state.autorotate = true;
                            break 'pref_indices_loop;
                        }
                        UserAction::NoAction => {
                            // Compare edges while waiting for key
                            for i in 0..edges_len {
                                if edges[i].diff_to.len() != 0 ||
                                   edges[i].solved_index != usize::max_value() {
                                    continue;
                                }
                                //println!("comparing {}/{}", i, edges_len);
                                compute_best_diff(i,
                                                  &mut edges,
                                                  combi_one_edge,
                                                  max_width,
                                                  max_height);
                                break;
                            }
                        }
                        _ => {
                            break 'display_and_precompute;
                        }
                    }
                }
            }
        }
    }
}
