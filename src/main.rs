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

#[derive(Copy, Clone)]
struct PieceInfo {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

impl PieceInfo {
    fn mid_x(&self) -> usize {
        return (self.min_x + self.max_x) / 2;
    }

    fn mid_y(&self) -> usize {
        return (self.min_y + self.max_y) / 2;
    }
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

    let mut pixels_ff: [[u8; MAX_HEIGHT]; MAX_WIDTH] = [[false; MAX_HEIGHT]; MAX_WIDTH];

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
fn compare_pieces(p1: &PieceInfo, p2: &PieceInfo, pixels: &mut [[u8; MAX_HEIGHT]; MAX_WIDTH]) -> u32 {

        for y in p1.min_y..p1.max_y + 1 {
            for x in p1.min_x..p1.max_x + 1 {
                if pixels[p1.max_x - x][y] != 0 {
                    println!("delta p1 {:?}", p1.max_x - x);
                    pixels[p1.max_x - x + 2][y] = true;
                    break;
                }
            }
        }

        for y in p2.min_y..p2.max_y + 1 {
            for x in p2.min_x..p2.max_x + 1 {
                if pixels[x][y] != 0 {
                    println!("delta p2 {:?}", x - p2.min_x);
                    pixels[x-2][y] = true;
                    break;
                }
            }
        }

        return 0;
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

    let mut pixels: [[u8; MAX_HEIGHT]; MAX_WIDTH] = [[0; MAX_HEIGHT]; MAX_WIDTH];

    // Image -> array
    for x in 0..MAX_WIDTH {
        for y in 0..MAX_HEIGHT {
            if x >= dims.0 as usize || y >= dims.1 as usize {
                continue;
            }
            let pix = im.get_pixel(x as u32, y as u32);
            if pix[0] < 127 {
                pixels[x as usize][y as usize] = 255;
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

    compare_pieces(&pcs[0], &pcs[1], &mut pixels);


    // Draw result bitmap
    let black_pix = image::Rgba([0, 0, 0, 0]);
    let grey_pix = image::Rgba([32, 32, 32, 0]);
    for x in 0..MAX_WIDTH {
        for y in 0..MAX_HEIGHT {
            if x >= dims.0 as usize || y >= dims.1 as usize {
                continue;
            }
            if pixels[x as usize][y as usize] != 0 {
                im.put_pixel(x as u32, y as u32, grey_pix); // paint with black/grey
            } else {
                im.put_pixel(x as u32, y as u32, black_pix);
            }
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

        /*for y in pi.min_y..pi.max_y + 1 {
            for x in pi.min_x..pi.max_x + 1 {
                if pixels[x][y] {
                    println!("delta piece {:?} = {:?}", p, x - pi.min_x);
                    break;
                }
            }
        }*/
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
