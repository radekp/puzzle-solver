extern crate sdl2;
extern crate image;

use std::fs;
use std::cmp;
use std::env;
use std::fs::File;
use std::path::Path;
use std::str::FromStr;
use std::error::Error;
use std::cmp::Ordering;
use std::io::prelude::*;
use std::collections::HashMap;

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
    edge_no: usize, // e.g. 103 is 10.3.txt
    edge_index: usize, // index to edges vector
    max_x: usize,
    max_y: usize,
    diff_to: Vec<usize>, // distance sum to edge at given index (in edges vector)
    best_diff: Vec<(usize, usize)>, // top 10 (edge_index, diff)
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
        let filename = format!("{}.{}.txt", png_no, side);
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
    return (a.max_x * a.max_y).cmp(&(b.max_x * b.max_y));
}

fn compare_edge_with_others2_helper(edge_e: &mut EdgeInfo,
                                    distances: &mut Vec<usize>,
                                    others: &mut [EdgeInfo],
                                    max_width: usize) {

    // One edge must be flipped
    let ref mut points_e = &flip_coords(&edge_e.points);

    for edge_o in others {
        let mut diff = 0;
        for o in edge_o.points.iter() {
            let offset = max_width * o.1 + o.0;
            let mut best_dst = distances[offset]; // precomputed distance for edge_e

            // Compute best distance to edge from given x,y (point e) on first hit
            if best_dst == usize::max_value() {

                for e in points_e.iter() {
                    let dx = (o.0 as isize) - (e.0 as isize);
                    let dy = (o.1 as isize) - (e.1 as isize);
                    let dst = (dx * dx + dy * dy) as usize;
                    if dst < best_dst {
                        best_dst = dst;
                    }
                }
                distances[offset] = best_dst;
            }
            diff += best_dst;
        }
        edge_e.diff_to[edge_o.edge_index] = diff; // so that we compare a->b
        //edge_o.diff_to[edge_e.edge_index] += diff; // and b->a
    }
}

fn compare_edge_with_others2(edges: &mut Vec<EdgeInfo>,
                             edge_index: usize,
                             max_width: usize,
                             max_height: usize) {

    if edges[edge_index].diff_to.len() > 0 {
        return;
    }
    let edges_len = edges.len();

    let (a, b) = edges.split_at_mut(edge_index);
    let (c, d) = b.split_at_mut(1);

    // For each x,y there is distance to edge at edge_index
    let mut distances = vec![usize::max_value();max_width*max_height];

    let ref mut edge = c[0];
    edge.diff_to = vec![0; edges_len];

    compare_edge_with_others2_helper(edge, &mut distances, a, max_width);
    compare_edge_with_others2_helper(edge, &mut distances, d, max_width);
}

// Compute egge.best_diff vector
fn compute_best_diff(i: usize,
                     edges: &mut Vec<EdgeInfo>,
                     num_best: usize,
                     max_width: usize,
                     max_height: usize) {

    if edges[i].best_diff.len() > 0 {
        return;
    }

    let edges_len = edges.len();

    // Init best_diff with 10 values
    let init_j = (i + 1) % edges_len;
    for _ in 0..num_best {
        edges[i].best_diff.push((init_j, usize::max_value()));
    }

    // We will take diff for each i->j compare
    for j in 0..edges[i].diff_to.len() {

        if i == j {
            continue; // dont compare with self
        }
        let diff_ij = edges[i].diff_to[j];
        let worst = edges[i].best_diff[num_best - 1].1;
        if diff_ij > worst {
            continue;
        }

        compare_edge_with_others2(edges, j, max_width, max_height);
        let diff_ji = edges[j].diff_to[i];
        let diff = diff_ij + diff_ji;

        for k in 0..edges[i].best_diff.len() {
            // index to best
            let mut b = edges[i].best_diff[k];
            if diff <= b.1 {
                edges[i].best_diff[k] = (j, diff); // replace best
                let mut kk = k + 1;
                while kk < edges[i].best_diff.len() {
                    // places the prev best after it
                    let tmp = edges[i].best_diff[kk];
                    edges[i].best_diff[kk] = b;
                    b = tmp;
                    kk += 1;
                }
                break;
            }
        }
    }

    /*let i_no = edges[i].edge_no;
    print!("best diffs for {}.{}: ", i_no >> 2, i_no & 3);
    for b in edges[i].best_diff.iter() {
        let b_no = edges[b.0].edge_no;
        print!("({}.{},{})", b_no >> 2, b_no & 3, b.1);
    }
    println!("");*/
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

        let filename_nums: usize = path.file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .replace(".", "")
            .parse()
            .unwrap();

        // 12.3.txt -> 123 -> 4 * 12 + 3
        let edge_no: usize = 4 * (filename_nums / 10) + (filename_nums % 10);

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
            edge_no: edge_no,
            max_x: max_x,
            max_y: max_y,
            diff_to: vec![],
            best_diff: vec![],
            edge_index: usize::max_value(),
        };
        edges.push(edge_info);
    }

    let edges_len = edges.len();

    // Max x and y in all edges, make diff_to vector
    let mut max_x = 0;
    let mut max_y = 0;
    for edge in edges.iter_mut() {
        max_x = cmp::max(max_x, edge.max_x);
        max_y = cmp::max(max_y, edge.max_y);
    }
    let max_width = max_x + 1;
    let max_height = max_y + 1;

    // SDL window - make it modulo 4 to play well with texture pitch
    let sqr = 2 * cmp::max(max_width, max_height) + 5 & !3usize;

    let mut pixels: Vec<u8> = vec![0;3*sqr*sqr];
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("puzzle solver", sqr as u32, sqr as u32)
        .position(0, 0)
        .opengl()
        .build()
        .unwrap();

    let mut renderer = window.renderer().build().unwrap();

    // Sort similar edges (by volume of widht*height)
    edges.sort_by(|a, b| compare_edge_info(a, b));

    // Prefer pieces from command line
    let mut pref_indices = vec![];
    for (index, arg) in env::args().enumerate() {
        if index == 0 {
            continue; // arg0 is path to exe file
        }
        let argv: usize = arg.parse().unwrap();
        for i in 0..edges_len {
            let png_no = edges[i].edge_no >> 2;
            if png_no == argv {
                pref_indices.push(i);
            }
        }
    }
    pref_indices.sort();
    for i in 0..pref_indices.len() {
        edges.swap(i, pref_indices[i]);
    }

    // Hashmap to get index by edge_no
    let mut edge_nums = HashMap::new();
    for i in 0..edges_len {
        let ref mut edge_i = edges[i];
        let i_no = edge_i.edge_no;
        println!("edge={}.{}", i_no >> 2, i_no & 3);
        edge_nums.insert(i_no, i);
        edge_i.edge_index = i;
    }

    /*for i in 0..edges_len {
        let edge_no = edges[i].edge_no;
        println!("comparing edge {:>4}.{} {}/{} with others",
                 edge_no >> 2,
                 edge_no & 3,
                 i,
                 edges_len);
        compare_edge_with_others2(&mut edges, i, max_width, max_height);
    }

    // Foreach edge index find top 10 best matching
    compute_best_diffs(&mut edges, 10);*/

    println!("Compared edges:");
    println!("");
    println!("   1st     2nd     3rd   4th          score");

    // Do 4-edges compare (edges from pieces A,B,C,D)
    //     C  ->  D
    //     ^      |
    //     |      v
    //     B  <-  A
    for a in 0..edges_len {
        let a_no = edges[a].edge_no;

        compare_edge_with_others2(&mut edges, a, max_width, max_height);
        compute_best_diff(a, &mut edges, 10, max_width, max_height);

        // Loop to compare combination of best edges, e.g. 1stJ..1stP, 1stJ..2ndM, 2ndJ..2ndM
        let mut best_final_score = usize::max_value();
        let mut combi_counter = 0;
        let mut best_combi_counter = 0;
        'combi_loop: loop {

            // Last round displays the best result
            let combi_val = if combi_counter <= 63 {
                combi_counter
            } else if combi_counter == 64 {
                println!("======= BEST MATCH {} ========", best_combi_counter);
                display_state.autorotate = false;
                best_combi_counter
            } else {
                break 'combi_loop;
            };

            let combi = (combi_val & 3, (combi_val >> 2) & 3, (combi_val >> 4) & 3);
            println!("combi_counter={} combi={}.{}.{}",
                     combi_counter,
                     combi.0,
                     combi.1,
                     combi.2);

            combi_counter += 1;

            //     B  <-  A
            let (b, diff_b) = edges[a].best_diff[combi.0];
            let b_no = edges[b].edge_no;
            compare_edge_with_others2(&mut edges, b, max_width, max_height);
            compute_best_diff(b, &mut edges, 10, max_width, max_height);


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

            compare_edge_with_others2(&mut edges, b_plus, max_width, max_height);
            compute_best_diff(b_plus, &mut edges, 10, max_width, max_height);


            let (c, diff_c) = edges[b_plus].best_diff[combi.1];
            let c_no = edges[c].edge_no;
            compare_edge_with_others2(&mut edges, c, max_width, max_height);
            compute_best_diff(c, &mut edges, 10, max_width, max_height);


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
            compare_edge_with_others2(&mut edges, c_plus, max_width, max_height);
            compute_best_diff(c_plus, &mut edges, 10, max_width, max_height);


            let (d, diff_d) = edges[c_plus].best_diff[combi.2];
            let d_no = edges[d].edge_no;
            compare_edge_with_others2(&mut edges, d, max_width, max_height);
            compute_best_diff(d, &mut edges, 10, max_width, max_height);


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
            let d_plus_no = side_plus(d_no);
            let d_plus = *edge_nums.get(&d_plus_no).unwrap();

            compare_edge_with_others2(&mut edges, d_plus, max_width, max_height);
            compute_best_diff(d_plus, &mut edges, 10, max_width, max_height);

            let a_minus_no = side_minus(a_no);
            let a_minus_ret = edge_nums.get(&a_minus_no);
            
            // The last edge can be marked as solved and thus not loaded
            if a_minus_ret.is_none() {
		continue;
            }
            
            let a_minus = *a_minus_ret.unwrap();

            compare_edge_with_others2(&mut edges, a_minus, max_width, max_height);
            compute_best_diff(a_minus, &mut edges, 10, max_width, max_height);

            let diff_a_minus = edges[a_minus].diff_to[d_plus];

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
            }

            // Display comapred edges
            for p in pixels.iter_mut() {
                *p = 0;
            }

            draw_coords(&mut pixels, sqr, &edges[a].points, 0, 0, 255, 0, 0);
            draw_coords(&mut pixels,
                        sqr,
                        &flip_coords(&edges[b].points),
                        0,
                        0,
                        0,
                        255,
                        0);

            draw_coords(&mut pixels, sqr, &edges[b_plus].points, 100, 0, 255, 0, 0);
            draw_coords(&mut pixels,
                        sqr,
                        &flip_coords(&edges[c].points),
                        100,
                        0,
                        0,
                        255,
                        0);

            draw_coords(&mut pixels, sqr, &edges[c_plus].points, 200, 0, 255, 0, 0);
            draw_coords(&mut pixels,
                        sqr,
                        &flip_coords(&edges[d].points),
                        200,
                        0,
                        0,
                        255,
                        0);

            draw_coords(&mut pixels, sqr, &edges[d_plus].points, 300, 0, 255, 0, 0);
            draw_coords(&mut pixels,
                        sqr,
                        &flip_coords(&edges[a_minus].points),
                        300,
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
                    write_done_file(&format!("data/{}.txt", a_no));
                    write_done_file(&format!("data/{}.txt", b_no));
                }
                _ => {}
            }
        }
    }
}
