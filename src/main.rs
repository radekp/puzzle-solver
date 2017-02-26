extern crate image;

use std::env;
use std::fs::File;
use std::path::Path;

use image::GenericImage;

fn get_nearby(img: &[[u8; 1000];500], x: usize, y:usize) -> u8 {
    if img[x][y] < 127 {
        return 0
    }
    return 1
}

fn flood_fill(src: &[[u8; 1000];500], dst: &[[u8; 1000];500], x: usize, y:usize) {
    
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

    let mut pieces: [[[u8; 1000];500];10] = [[[0; 1000]; 500];10];

    println!("res={:?}", get_nearby(&pieces[0], 10, 10));

    // Image -> array
	for x in 0..1000 {
		for y in 0..500 {
            if x >= dims.0 || y >= dims.1 {
                continue;
            }
            let pix = im.get_pixel(x, y);
            if pix[0] > 127 {
                pieces[0][y as usize][x as usize] = 255;
            }
		}
	}

    // Split pieces
    for x in 0..1000 {
        for y in 0..500 {
            if pieces[0][y][x] < 127 {
                continue
            }

        }
    }


    for x in 0..1000 {
		for y in 0..500 {
            if x >= dims.0 || y >= dims.1 {
                continue;
            }
            let v = pieces[0][y as usize][x as usize] / 10;     // paint with black/grey
            let pix = image::Rgba([v,v,v,0]);
            im.put_pixel(x, y, pix);
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
}
