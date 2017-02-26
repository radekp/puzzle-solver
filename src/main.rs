extern crate image;

use std::env;
use std::fs::File;
use std::path::Path;

use image::GenericImage;

// Move points from src to dst recursively with flood fill
fn flood_fill(pieces: &mut[[[bool; 500];1000];10], p: usize, x: usize, y:usize) {

    if !pieces[0][x][y] {
        return;
    }
    pieces[0][x][y] = false;
    pieces[p][x][y] = true;
    if x > 0 {
        flood_fill(pieces, p, x-1, y);
    }
    if y > 0 {
        flood_fill(pieces, p, x, y-1);
    }
    if x < 1000 {
        flood_fill(pieces, p, x+1, y);
    }
    if y < 500 {
        flood_fill(pieces, p, x, y+1);
    }
}

// Split pieces
fn split_pieces(pieces: &mut[[[bool; 500];1000];10]) {
    let mut p = 1;
        for x in 0..1000 {
            for y in 0..500 {
                if !pieces[0][x][y] {
                    continue
                }
                flood_fill(pieces, p, x, y);
                p = p + 1;
                if p >= 10 {
                    return;
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

    let mut pieces: [[[bool; 500];1000];10] = [[[false; 500]; 1000];10];

    // Image -> array
	for x in 0..1000 {
		for y in 0..500 {
            if x >= dims.0 || y >= dims.1 {
                continue;
            }
            let pix = im.get_pixel(x, y);
            if pix[0] < 127 {
                pieces[0][x as usize][y as usize] = true;
            }
		}
	}

    split_pieces(&mut pieces);


    let black_pix = image::Rgba([0,0,0,0]);
    let grey_pix = image::Rgba([32,32,32,0]);
    for x in 0..1000 {
		for y in 0..500 {
            if x >= dims.0 || y >= dims.1 {
                continue;
            }
            if pieces[2][x as usize][y as usize] {
                im.put_pixel(x, y, grey_pix);           // paint with black/grey
            } else {
                im.put_pixel(x, y, black_pix);
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
