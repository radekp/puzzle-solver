extern crate image;

use std::env;
use std::fs::File;
use std::path::Path;

use image::GenericImage;

// Maximal wifth/height for pieces array
const MAX_WIDTH: usize = 512;
const MAX_HEIGHT: usize = 213;

// Maximum number of pieces
const MAX_PIECES: usize = 12;

struct PieceInfo {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
    pixels: [[bool; MAX_HEIGHT]; MAX_WIDTH],
}

// Move points from src to dst recursively with flood fill
fn flood_fill(pieces: &mut [[[bool; MAX_HEIGHT]; MAX_WIDTH]; MAX_PIECES],
              p: usize,
              x: usize,
              y: usize,
              pi: &mut PieceInfo)
              -> u32 {

    if !pieces[0][x][y] {
        return 0;
    }
    pieces[0][x][y] = false;
    pieces[p][x][y] = true;
    pi.pixels[x][y] = true;

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
        res = res + flood_fill(pieces, p, x - 1, y, pi);
    }
    if y > 0 {
        res = res + flood_fill(pieces, p, x, y - 1, pi);
    }
    if x < MAX_WIDTH {
        res = res + flood_fill(pieces, p, x + 1, y, pi);
    }
    if y < MAX_HEIGHT {
        res = res + flood_fill(pieces, p, x, y + 1, pi);
    }
    return res;
}

// Split pieces
fn split_pieces(pieces: &mut [[[bool; MAX_HEIGHT]; MAX_WIDTH]; MAX_PIECES]) {
    let mut p = 1;
    for x in 0..MAX_WIDTH {
        for y in 0..MAX_HEIGHT {
            if !pieces[0][x][y] {
                continue;
            }
            let mut pi = PieceInfo{min_x: usize::max_value(), min_y: usize::max_value(), max_x: 0, max_y:0,
                pixels:[[false; MAX_HEIGHT]; MAX_WIDTH]};

            let num_pix = flood_fill(pieces, p, x, y, &mut pi);
            println!("piece {:?} numPix={:?} min={:?},{:?} max={:?},{:?}", p, num_pix, pi.min_x, pi.min_y, pi.max_x, pi.max_y);
            if num_pix == 1 {
                pieces[p][x][y] = false;
                continue;
            }

            p = p + 1;
            if p >= MAX_PIECES {
                return;
            }
        }
    }
}

fn detect_edge(pieces: &mut [[[bool; MAX_HEIGHT]; MAX_WIDTH]; MAX_PIECES]) {
    for p in 1..MAX_PIECES {
        for x in 0..MAX_WIDTH {
            for y in 0..MAX_HEIGHT {
            }
        }
    }
}

fn main() {

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

    let mut pieces: [[[bool; MAX_HEIGHT]; MAX_WIDTH]; MAX_PIECES] =
        [[[false; MAX_HEIGHT]; MAX_WIDTH]; MAX_PIECES];

    // Image -> array
    for x in 0..MAX_WIDTH {
        for y in 0..MAX_HEIGHT {
            if x >= dims.0 as usize || y >= dims.1 as usize {
                continue;
            }
            let pix = im.get_pixel(x as u32, y as u32);
            if pix[0] < 127 {
                pieces[0][x as usize][y as usize] = true;
            }
        }
    }

    split_pieces(&mut pieces);


    // Draw result bitmap
    let black_pix = image::Rgba([0, 0, 0, 0]);
    for p in 1..MAX_PIECES {
        let grey_pix = image::Rgba([32, 32, 32, 0]);
        for x in 0..MAX_WIDTH {
            for y in 0..MAX_HEIGHT {
                if x >= dims.0 as usize || y >= dims.1 as usize {
                    continue;
                }
                if pieces[p][x as usize][y as usize] {
                    im.put_pixel(x as u32, y as u32, grey_pix); // paint with black/grey
                } else if p == 1 {
                    im.put_pixel(x as u32, y as u32, black_pix);
                }
            }
        }
    }

    // Detect edge
    for p in 1..MAX_PIECES {
        let white_pix = image::Rgba([0, 255, 0, 0]);
        for y in 0..MAX_HEIGHT {
            for x in 0..MAX_WIDTH {
                if x >= dims.0 as usize || y >= dims.1 as usize {
                    continue;
                }
                if pieces[p][x as usize][y as usize] {
                    im.put_pixel(x as u32, y as u32, white_pix);
                    break;
                }
            }
        }
    }


    // +----
    // |/ /
    // |/
    /*let mut prevEdgeDetected = false;
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
	}*/



    let ref mut fout = File::create(&Path::new(&format!("{}.png", file))).unwrap();

    // Write the contents of this image to the Writer in PNG format.
    let _ = im.save(fout, image::PNG).unwrap();
}
