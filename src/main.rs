extern crate sdl2;
extern crate image;

use std::env;
use std::cmp;
use std::fs::File;
use std::path::Path;

use image::GenericImage;

use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::image::LoadTexture;
use sdl2::render::TextureQuery;

// Maximal wifth/height for pieces array
const MAX_WIDTH: usize = 512;
const MAX_HEIGHT: usize = 213;

// Maximum number of pieces
const MAX_PIECES: usize = 12;

#[derive(Copy, Clone)]
struct PieceInfo {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

impl PieceInfo {
    /*    fn mid_x(&self) -> usize {
        return (self.min_x + self.max_x) / 2;
    }

    fn mid_y(&self) -> usize {
        return (self.min_y + self.max_y) / 2;
    }*/

    fn width(&self) -> usize {
        return self.max_x - self.min_x;
    }

    fn height(&self) -> usize {
        return self.max_y - self.min_y;
    }
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

// Move points from src to dst recursively with flood fill
fn flood_fill(pixels: &mut [[u8; MAX_HEIGHT]; MAX_WIDTH],
              x: usize,
              y: usize,
              pi: &mut PieceInfo)
              -> u32 {

    if pixels[x][y] == 0 {
        return 0;
    }
    pixels[x][y] = 0;

    // Update min & max points
    if x > pi.max_x {
        pi.max_x = x;
    }
    if y > pi.max_y {
        pi.max_y = y;
    }
    if x < pi.min_x {
        pi.min_x = x;
    }
    if y < pi.min_y {
        pi.min_y = y;
    }

    let mut res: u32 = 1;
    if x > 0 {
        res = res + flood_fill(pixels, x - 1, y, pi);
    }
    if y > 0 {
        res = res + flood_fill(pixels, x, y - 1, pi);
    }
    if x < MAX_WIDTH {
        res = res + flood_fill(pixels, x + 1, y, pi);
    }
    if y < MAX_HEIGHT {
        res = res + flood_fill(pixels, x, y + 1, pi);
    }
    return res;
}

// Split pieces
fn split_pieces(pcs: &mut [PieceInfo; MAX_PIECES],
                pixels: &mut [[u8; MAX_HEIGHT]; MAX_WIDTH])
                -> usize {

    let mut pixels_ff: [[u8; MAX_HEIGHT]; MAX_WIDTH] = [[0; MAX_HEIGHT]; MAX_WIDTH];

    for x in 0..MAX_WIDTH {
        for y in 0..MAX_HEIGHT {
            pixels_ff[x][y] = pixels[x][y];
        }
    }

    let mut p = 0;
    for x in 0..MAX_WIDTH {
        for y in 0..MAX_HEIGHT {
            if pixels_ff[x][y] == 0 {
                continue;
            }
            let mut pi = PieceInfo {
                min_x: usize::max_value(),
                min_y: usize::max_value(),
                max_x: 0,
                max_y: 0,
            };

            let num_pix = flood_fill(&mut pixels_ff, x, y, &mut pi);
            println!("piece {:?} numPix={:?} min={:?},{:?} max={:?},{:?}",
                     p,
                     num_pix,
                     pi.min_x,
                     pi.min_y,
                     pi.max_x,
                     pi.max_y);

            if num_pix == 1 {
                pixels_ff[x][y] = 0;
                continue;
            }
            pi.min_x -= 3; // some space for comparing pieces
            pi.min_y -= 3;
            pi.max_x += 20;
            pi.max_y += 3;
            pcs[p] = pi;

            p = p + 1;
            if p >= MAX_PIECES {
                return MAX_PIECES;
            }
        }
    }
    return p;
}

// Compare two pieces and return score
fn compare_pieces_x_y_rot(p1: &PieceInfo,
                          p2: &PieceInfo,
                          pixels: &mut [[u8; MAX_HEIGHT]; MAX_WIDTH],
                          delta_x: i32,
                          delta_y: i32,
                          rotate: i32)
                          -> i32 {

    // Clear previous comparing
    for y in p1.min_y..p1.max_y + 1 {
        for x in p1.min_x..p1.max_x + 1 {
            pixels[x][y] &= 32;
        }
    }

    let width = cmp::min(p1.width(), p2.width());
    let height = cmp::min(p1.height(), p2.height());
    let iheight = height as i32;

    // Move p2 piece to p1 area.
    for y in 0..height {
        for x in 0..width {

            let x2 = p2.min_x + x;
            let y2 = p2.min_y + y;

            if pixels[x2][y2] == 0 {
                // empty p2 pixel
                continue;
            }

            let ix = x as i32;
            let iy = y as i32;

            let ix1 = delta_x + (p1.min_x as i32 + ix) + ((rotate * iy) / iheight);
            let iy1 = delta_y + (p1.min_y + y) as i32;

            if ix1 < 0 || iy1 < 0 {
                continue;
            }

            let x1 = ix1 as usize;
            let y1 = iy1 as usize;

            if x1 >= p1.max_x || y1 >= p1.max_y {
                continue;
            }

            if pixels[x1][y1] != 0 {
                // intersection with p1
                pixels[x1][y1] |= 128;
            } else {
                pixels[x1][y1] |= 64; // no intersection, just draw p2
            }
        }
    }


    // Compute score
    let mut res: i32 = 0;
    for y in p1.min_y..p1.max_y {
        for x in p1.min_x..p1.max_x {
            if pixels[x][y] & 128 == 0 {
                continue; // skip all but intersection
            }

            // Find nearest point in piece p1 and p2
            let mut iter = near_iter_begin(x as i32, y as i32, 1);
            let mut dist_1 = 0;
            let mut dist_2 = 0;
            //println!("iter_begin x={:?} y={:?} a={:?}", iter.0, iter.1, iter.2);
            loop {
                //println!("x={:?} y={:?} a={:?} pix={:?}",
                // iter.0, iter.1, iter.2, pixels[iter.0 as usize][iter.1 as usize]);
                if iter.0 >= p1.min_x as i32 && iter.0 <= p1.max_x as i32 &&
                   iter.1 >= p1.min_y as i32 && iter.1 <= p1.max_y as i32 {
                    if pixels[iter.0 as usize][iter.1 as usize] == 32 && dist_1 == 0 {
                        dist_1 = iter.2;
                    }
                    if pixels[iter.0 as usize][iter.1 as usize] == 64 && dist_2 == 0 {
                        dist_2 = iter.2;
                    }
                    if dist_1 > 0 && dist_2 > 0 {
                        break;
                    }
                }
                iter = near_iter_next(x as i32, y as i32, iter.0, iter.1, iter.2);
            }

            // Close point is positive score, distant is negative
            res += 3 - dist_1 - dist_2;
            if iheight + res < 0 {
                return res; // bail out early when score is too bad
            }
        }
    }
    return res;
}

fn compare_pieces(p1: &PieceInfo,
                  p2: &PieceInfo,
                  pixels: &mut [[u8; MAX_HEIGHT]; MAX_WIDTH])
                  -> (i32, i32, i32) {

    let mut best_score: i32 = 0;
    let mut best_x: i32 = 0;
    let mut best_y: i32 = 0;
    let mut best_r: i32 = 0;

    for r in -6..6 {
        // fake rotation +-6pixels
        for y in 0..p1.height() / 2 {
            for x in p1.width() / 2..p1.width() {
                // move less then half p1 width never fits
                let score = compare_pieces_x_y_rot(p1, p2, pixels, x as i32, y as i32, r);
                if score > best_score {
                    best_score = score;
                    best_x = x as i32;
                    best_y = y as i32;
                    best_r = r as i32;
                    println!("x={:?} y={:?} r={:?} height={:?} score={:?}",
                             x,
                             y,
                             r,
                             p1.height(),
                             score);
                }
            }
        }
    }
    return (best_x, best_y, best_r);
}

fn main() {

    let sdl_context = sdl2::init().unwrap();

    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("rust-sdl2 demo: Video", 800, 600)
        .position_centered()
        .opengl()
        .build()
        .unwrap();

    let mut renderer = window.renderer().build().unwrap();
    let mut texture = renderer.load_texture("1.jpg").unwrap();

    let TextureQuery { width, height, .. } = texture.query();

    println!("{}x{}", width, height);

    // Some space so that rotation does not crop image
    let shift = (cmp::max(width, height) / 3) as i32;

    'rotating: for r in 0..360 {

        renderer.clear();
        renderer.copy_ex(&texture,
                     None,
                     Some(Rect::new(shift, shift, width, height)),
                     r as f64,
                     None,
                     false,
                     false)
            .unwrap();

        //renderer.present();

        let mut pixels =
            renderer.read_pixels(Some(Rect::new(0, 0, 800, 600)), PixelFormatEnum::RGB24)
                .unwrap();

        // Detect piece
        for y in 0..600 {
            for x in 0..800 {
                let offset = 3 * (800 * y + x);
                let r = pixels[offset] as i32;
                let b = pixels[offset + 2] as i32;
                if b - r > 30 {
                    pixels[offset] = 255;
                    pixels[offset + 1] = 255;
                    pixels[offset + 2] = 255;
                } else {
                    pixels[offset] = 0;
                    pixels[offset + 1] = 0;
                    pixels[offset + 2] = 0;
                }
            }
        }

        // Find corner
        let mut iter = near_iter_begin(0, 0, 1);
        loop {
            if iter.0 >= 0 && iter.0 < 800 && iter.1 >= 0 && iter.1 < 600 {
                let offset = 3 * (800 * iter.1 + iter.0) as usize;
                if pixels[offset] == 255 {
                    pixels[offset] = 0;
                    pixels[offset+2] = 0;
                    println!("{},{}", iter.0, iter.1);
                    break;
                }
                pixels[offset] = 128;
            }
            iter = near_iter_next(0, 0, iter.0, iter.1, iter.2);
        }


        let mut texture2 = renderer.create_texture_streaming(PixelFormatEnum::RGB24, 800, 600)
            .unwrap();

        // Create a red-green gradient
        let mut index = 0;
        texture2.with_lock(None, |buffer: &mut [u8], pitch: usize| for y in 0..600 {
                for x in 0..800 {
                    let offset = y * pitch + x * 3;
                    buffer[offset + 0] = pixels[offset];
                    buffer[offset + 1] = pixels[offset + 1];
                    buffer[offset + 2] = pixels[offset + 2];
                    index += 1;
                }
            })
            .unwrap();

        renderer.clear();
        renderer.copy(&texture2, None, None)
            .unwrap();
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
}

/*
fn main_old() {


    let file = if env::args().count() == 2 {
        env::args().nth(1).unwrap()
    } else {
        panic!("Please enter a file")
    };

    // Use the open function to load an image from a PAth.
    // ```open``` returns a dynamic image.
    let mut im = image::open(&Path::new(&file)).unwrap();

    let dims = im.dimensions();

    // The dimensions method returns the images width and height
    println!("dimensions {:?}", dims);

    let mut pixels: [[u8; MAX_HEIGHT]; MAX_WIDTH] = [[0; MAX_HEIGHT]; MAX_WIDTH];

    // Image -> array
    for x in 0..MAX_WIDTH {
        for y in 0..MAX_HEIGHT {
            if x >= dims.0 as usize || y >= dims.1 as usize {
                continue;
            }
            let pix = im.get_pixel(x as u32, y as u32);
            if pix[0] < 127 {
                pixels[x as usize][y as usize] = 32;
            }
        }
    }

    let mut pcs: [PieceInfo; MAX_PIECES] = [PieceInfo {
        min_x: usize::max_value(),
        min_y: usize::max_value(),
        max_x: 0,
        max_y: 0,
    }; MAX_PIECES];

    let num_pieces = split_pieces(&mut pcs, &mut pixels);

    let cmp = compare_pieces(&pcs[0], &pcs[1], &mut pixels);
    compare_pieces_x_y_rot(&pcs[0], &pcs[1], &mut pixels, cmp.0, cmp.1, cmp.2);

    //let score = compare_pieces_x_y_rot(&pcs[7], &pcs[9], &mut pixels, 39, 16, -1);
    //let score = compare_pieces_x_y_rot(&pcs[0], &pcs[1], &mut pixels, 0, 0, -5);
    //println!("score {:?}", score);

    // Draw result bitmap
    for x in 0..MAX_WIDTH {
        for y in 0..MAX_HEIGHT {
            if x >= dims.0 as usize || y >= dims.1 as usize {
                continue;
            }
            let c = pixels[x][y];
            let pix = image::Rgba([c, c, c, 0]);
            im.put_pixel(x as u32, y as u32, pix);
        }
    }


    let green_pix = image::Rgba([0, 255, 0, 0]);
    let red_pix = image::Rgba([255, 0, 0, 0]);
    let blue_pix = image::Rgba([0, 0, 255, 0]);
    for p in 0..num_pieces {
        let pi = pcs[p];
        println!("draw piece {:?} min={:?},{:?} max={:?},{:?}",
                 p,
                 pi.min_x,
                 pi.min_y,
                 pi.max_x,
                 pi.max_y);

        im.put_pixel(pi.min_x as u32, pi.min_y as u32, red_pix);
        im.put_pixel(pi.max_x as u32, pi.max_y as u32, green_pix);
        im.put_pixel(pi.mid_x() as u32, pi.mid_y() as u32, blue_pix);

        for y in pi.min_y..pi.max_y + 1 {
            for x in pi.min_x..pi.max_x + 1 {
                if pixels[x][y] {
                    println!("delta piece {:?} = {:?}", p, x - pi.min_x);
                    break;
                }
            }
        }
    }


    // Detect edge
    for p in 0..num_pieces {
        let pi = pcs[p];
        for y in pi.min_y..pi.max_y + 1 {
            for x in pi.min_x..pi.max_x + 1 {
                if x >= dims.0 as usize || y >= dims.1 as usize {
                    continue;
                }
                if pixels[x as usize][y as usize] != 0 {
                    im.put_pixel(x as u32, y as u32, green_pix);
                    break;
                }
            }
        }
        for x in pi.min_x..pi.max_x + 1 {
            for y in pi.min_y..pi.max_y + 1 {
                if x >= dims.0 as usize || y >= dims.1 as usize {
                    continue;
                }
                if pixels[x as usize][y as usize] != 0{
                    im.put_pixel(x as u32, y as u32, red_pix);
                    break;
                }
            }
        }
    }

    // +----
    // |/ /
    // |/
    let mut prevEdgeDetected = false;
    for i in 0..1500 {
        let mut edgeDetected = false;
		for j in 0..500 {
            let mut x:i32 = i + j - 500;
            let y = j;
            if x < 0 || x >= dims.0 as i32 || y >= dims.1 as i32 {
                continue;
            }
            let v = pieces[0][y as usize][x as usize];
            if v > 127 {
                continue;
            }
            let prevV = pieces[0][(y - 1) as usize][(x - 1) as usize];
            if prevV == v {
                continue;
            }

            let pix = image::Rgba([255,0,255,0]);
            im.put_pixel(x as u32, y as u32, pix);
            println!("x={:?} y={:?}", x, y);
            break;
		}
        prevEdgeDetected = edgeDetected;
	}



    let ref mut fout = File::create(&Path::new(&format!("{}.png", file))).unwrap();

    // Write the contents of this image to the Writer in PNG format.
    let _ = im.save(fout, image::PNG).unwrap();


   let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("rust-sdl2 demo: Video", 800, 600)
        .position_centered()
        .opengl()
        .build()
        .unwrap();

    let mut renderer = window.renderer().build().unwrap();

    let mut texture2 = renderer.load_texture("IMG_20170225_152806.jpg").unwrap();

    let mut texture = renderer.create_texture_streaming(
        PixelFormatEnum::RGB24, MAX_WIDTH as u32, MAX_HEIGHT as u32).unwrap();
    // Create a red-green gradient
    texture.with_lock(None, |buffer: &mut [u8], pitch: usize| {
        for y in 0..MAX_HEIGHT {
            for x in 0..MAX_WIDTH {
                let offset = y*pitch + x*3;
                buffer[offset + 0] = pixels[x][y];
                buffer[offset + 1] = pixels[x][y];
                buffer[offset + 2] = pixels[x][y];
            }
        }
    }).unwrap();

    renderer.clear();
    renderer.copy(&texture, None, Some(Rect::new(0, 0, MAX_WIDTH as u32, MAX_HEIGHT as u32))).unwrap();
    renderer.copy(&texture2, None, Some(Rect::new(10, 0, MAX_WIDTH as u32, MAX_HEIGHT as u32))).unwrap();
    renderer.present();

    let rpix = renderer.read_pixels(Some(Rect::new(0, 0, MAX_WIDTH as u32, MAX_HEIGHT as u32)), PixelFormatEnum::RGB24);
    println!("{:?}", rpix);

    let mut event_pump = sdl_context.event_pump().unwrap();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..}
                | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running
                },
                _ => {}
            }
        }
        // The rest of the game loop goes here...
    }
}*/
