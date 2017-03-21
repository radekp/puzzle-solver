extern crate sdl2;
extern crate image;

use std::fs;
use std::cmp;
use std::fs::File;
use std::path::Path;
use std::str::FromStr;
use std::error::Error;
use std::io::prelude::*;

use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::image::LoadTexture;
use sdl2::render::TextureQuery;
use sdl2::render::Renderer;
use sdl2::render::Texture;

// SDL window size - puzzle pieces bitmap must fit even with rotation
const RED_MASK_MATERIAL: u8 = 1 << 4;
const RED_MASK_BORDER: u8 = 1 << 7;
const RED_MASK_NO_BORDER: u8 = 1 << 1;
const RED_MASK_JAG: u8 = 1 << 5;

const GREEN_MASK_EDGE_1: u8 = 1 << 5;
const GREEN_MASK_EDGE_2: u8 = 1 << 7;

#[derive(Copy, Clone)]
struct URect {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

// Detect piece color - in my case they are dark blue
fn detect_material(pixels: &mut Vec<u8>, sqr: usize, x: usize, y: usize) -> bool {
    let offset = 3 * (sqr * y + x);
    let r = pixels[offset] as i32;
    let g = pixels[offset + 1] as i32;
    let b = pixels[offset + 2] as i32;
    if r + g + b > 3 * 127 {
        pixels[offset] = RED_MASK_MATERIAL;
        pixels[offset + 1] = RED_MASK_MATERIAL;
        pixels[offset + 2] = RED_MASK_MATERIAL;
        return true;
    }
    pixels[offset] = 0;
    pixels[offset + 1] = 0;
    pixels[offset + 2] = 0;
    return false;
}

// Draw border pixels with RED_MASK_BORDER
fn detect_border(pixels: &mut Vec<u8>, sqr: usize, x: usize, y: usize) -> bool {

    let offset = 3 * (sqr * y + x);
    let pix = pixels[offset];
    
    if pix & RED_MASK_NO_BORDER != 0 || pix & RED_MASK_BORDER != 0 {		// already processed this pixel?
        return false;
    }
    
    let mut near = false;
    if x > 0 {
        let offset_xm = 3 * (sqr * y + x - 1);
        if pixels[offset_xm] & RED_MASK_NO_BORDER != 0 {
            near = true;
        }
    }
    if y > 0 {
        let offset_ym = 3 * (sqr * (y - 1) + x);
        if pixels[offset_ym] & RED_MASK_NO_BORDER != 0 {
            near = true;
        }
    }
    let offset_xp = 3 * (sqr * y + x + 1);
    if pixels[offset_xp] & RED_MASK_NO_BORDER != 0 {
        near = true;
    }
    let offset_yp = 3 * (sqr * (y + 1) + x);
    if pixels[offset_yp] & RED_MASK_NO_BORDER != 0 {
        near = true;
    }
    
    // Nothing near where border is detected
    if !near {
      return false;
    }
    
    if pix & RED_MASK_MATERIAL == 0 {    
      pixels[offset] |= RED_MASK_NO_BORDER;
    } else {
      pixels[offset] |= RED_MASK_BORDER;
    }
    return true;
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
                           -> (usize, usize, usize, usize, Vec<u8>) {

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

    let mut pixels = renderer.read_pixels(Some(Rect::new(0, 0, sqr as u32, sqr as u32)),
                                          PixelFormatEnum::RGB24)
        .unwrap();

    // Detect piece and bounds
    let mut bounds = URect {
        min_x: usize::max_value(),
        min_y: usize::max_value(),
        max_x: 0,
        max_y: 0,
    };

    for y in 0..sqr {
        for x in 0..sqr {
            if !detect_material(&mut pixels, sqr, x, y) {
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
    let offset = 3 * (sqr * bounds.min_y + bounds.min_x);
    pixels[offset] |= RED_MASK_NO_BORDER;
    loop {
        let mut detected = false;
        for y in bounds.min_y-1..bounds.max_y+1 {
            for x in bounds.min_x-1..bounds.max_x+1 {
                detected |= detect_border(&mut pixels, sqr, x, y);
            }
        }
        if !detected {
            break;
        }
    }

    // Find jags that could spoil finding corners
    detect_jags(&mut pixels, sqr, bounds, sqr / 32, sqr / 6, sqr / 6);

    let rv = find_corners(&mut pixels, sqr, bounds, draw_corners);

    return (rv.0, rv.1, rv.2, rv.3, pixels);
}

fn fill_edge_rec(pixels: &mut Vec<u8>,
                 sqr: usize,
                 edge1: &mut Vec<(usize, usize)>,
                 edge2: &mut Vec<(usize, usize)>,
                 x: usize,
                 y: usize,
                 top_x: usize,
                 top_y: usize,
                 col: &mut u8) {

    let offset = 3 * (sqr * y + x);
    if pixels[offset] & RED_MASK_BORDER == 0 {
        // not border
        return;
    }
    if pixels[offset + 1] & GREEN_MASK_EDGE_1 != 0 {
        // already filled
        return;
    }
    if pixels[offset + 1] & GREEN_MASK_EDGE_2 != 0 {
        // already filled
        return;
    }
    pixels[offset + 1] |= *col;
    if *col == GREEN_MASK_EDGE_1 {
        edge1.push((x, y));
    } else {
        edge2.push((x, y));
    }

    if x == top_x && y == top_y {
        // reached the second corner
        *col = GREEN_MASK_EDGE_2; // swap color
        return;
    }

    fill_edge_rec(pixels, sqr, edge1, edge2, x + 1, y, top_x, top_y, col);
    fill_edge_rec(pixels, sqr, edge1, edge2, x - 1, y, top_x, top_y, col);
    fill_edge_rec(pixels, sqr, edge1, edge2, x, y + 1, top_x, top_y, col);
    fill_edge_rec(pixels, sqr, edge1, edge2, x, y - 1, top_x, top_y, col);
    fill_edge_rec(pixels, sqr, edge1, edge2, x + 1, y + 1, top_x, top_y, col);
    fill_edge_rec(pixels, sqr, edge1, edge2, x - 1, y + 1, top_x, top_y, col);
    fill_edge_rec(pixels, sqr, edge1, edge2, x + 1, y - 1, top_x, top_y, col);
    fill_edge_rec(pixels, sqr, edge1, edge2, x - 1, y - 1, top_x, top_y, col);
}

fn find_edge(pixels: &mut Vec<u8>,
             sqr: usize,
             top_x: usize,
             top_y: usize,
             bot_x: usize,
             bot_y: usize)
             -> String {

    let mut edge1 = vec![];
    let mut edge2 = vec![];

    fill_edge_rec(pixels,
                  sqr,
                  &mut edge1,
                  &mut edge2,
                  bot_x,
                  bot_y,
                  top_x,
                  top_y,
                  &mut GREEN_MASK_EDGE_1);

    println!("edge1={} edge2={}", edge1.len(), edge2.len());

    if edge1.len() > edge2.len() {
        edge1 = edge2;
    }

    // Find min
    let mut min_x = usize::max_value();
    let mut min_y = usize::max_value();
    for p in edge1.iter() {
        min_x = cmp::min(p.0, min_x);
        min_y = cmp::min(p.1, min_y);
    }

    let mut res: String = "".to_string();
    for p in edge1.iter() {
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
}

fn display_pixels(pixels: &Vec<u8>,
                  sqr: usize,
                  sdl_context: &sdl2::Sdl,
                  renderer: &mut Renderer)
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

    renderer.clear();
    renderer.copy(&res_texture, None, None).unwrap();
    renderer.present();

    //return UserAction::Rotate;

    let mut event_pump = sdl_context.event_pump().unwrap();

    loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::KeyDown { keycode: Some(Keycode::R), .. } => return UserAction::Rotate,
                Event::Quit { .. } |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => return UserAction::Quit,
                _ => {}
            }
        }
        // The rest of the game loop goes here...
    }
}

fn process_jpg(img_file: &str, sdl_context: &sdl2::Sdl) {

    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window(img_file, 800, 800)
        .position(100, 0)
        .opengl()
        .build()
        .unwrap();

    let mut renderer = window.renderer().build().unwrap();

    let texture = renderer.load_texture(img_file).unwrap();

    let TextureQuery { width, height, .. } = texture.query();

    let wnd_size = renderer.window().unwrap().size();
    if width >= wnd_size.0 || height >= wnd_size.1 {
        panic!("{} too big {}x{} window is just {}x{}",
               img_file,
               width,
               height,
               wnd_size.0,
               wnd_size.1);
    }

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

    // Resize window
    renderer.window_mut()
        .unwrap()
        .set_size(sqr as u32, sqr as u32)
        .unwrap();

    for side in 0..4 {

        let mut best_corner_delta = usize::max_value();
        let mut best_corner_angle = 0;

        'rotating: for r in -10..11 {

            let angle = 90 * side + r;
            //println!("angle={}", angle);

            let rv = rotate_and_find_corners(&mut renderer,
                                             &texture,
                                             angle as f64,
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

            match display_pixels(&pixels, sqr, sdl_context, &mut renderer) {
                UserAction::Quit => break 'rotating,
                _ => {}
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

        // Save left edge coordinates to file
        let content = find_edge(&mut pixels, sqr, top_x, top_y, bot_x, bot_y);
        let ext = format!("{}.txt", side);
        let txt_path = Path::new(img_file).with_extension(ext);
        let display = txt_path.display();

        let mut file = match File::create(&txt_path) {
            Err(why) => panic!("couldn't create {}: {}", display, why.description()),
            Ok(file) => file,
        };

        match file.write_all(content.as_bytes()) {
            Err(why) => panic!("couldn't write to {}: {}", display, why.description()),
            Ok(_) => println!("successfully wrote to {}", display),
        }

        display_pixels(&pixels, sqr, sdl_context, &mut renderer);
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
               g: u8) {
    for p in coords {
        let x = p.0 + left;
        let y = p.1 + top;
        let offset = 3 * (sqr * y + x);
        pixels[offset] = r;
        pixels[offset + 1] = g;
    }
}

fn flip_coords(coords: &Vec<(usize, usize)>) -> Vec<(usize, usize)> {

    let mut max_x = 0;
    let mut max_y = 0;

    for p in coords {
        max_x = cmp::max(p.0, max_x);
        max_y = cmp::max(p.1, max_y);
    }

    println!("max={} {}", max_x, max_y);

    let mut res = vec![];
    for p in coords {
        res.push((max_x - p.0, max_y - p.1));
    }
    return res;
}

fn main() {

    let sdl_context = sdl2::init().unwrap();


    let paths = fs::read_dir("./").unwrap();
    for path in paths {
        //println!("Name: {}", path.unwrap().path().into_os_string().into_string());
        let path_str = path.unwrap()
            .path()
            .into_os_string()
            .into_string()
            .unwrap();
        if !path_str.ends_with(".jpg") {
            continue;
        }
        process_jpg(&path_str, &sdl_context);
    }

    //process_jpg("9.jpg", &sdl_context);

    /*let mut pixels: Vec<u8> = vec![0;3*WND_WIDTH*WND_HEIGHT];

    //draw_coords(&mut pixels, &read_txt("2.0.txt"), 0, 0);
    draw_coords(&mut pixels, &read_txt("9.0.txt"), 100, 100, 255, 0);
    draw_coords(&mut pixels, &flip_coords(&read_txt("10.2.txt")), 100, 100, 0, 255);

    let video_subsystem = sdl_context.video().unwrap();

    let window =
        video_subsystem.window("rust-sdl2 demo: Video", 800, 600)
            .position(100,0)
            .opengl()
            .build()
            .unwrap();

    let mut renderer = window.renderer().build().unwrap();


    display_pixels(&pixels, 800, &sdl_context, &mut renderer);*/
}
