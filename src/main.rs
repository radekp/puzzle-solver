extern crate image;

use std::env;
use std::fs::File;
use std::path::Path;

use image::GenericImage;

fn get_nearby(img: &[[u8; 500];500], x: usize, y:usize) -> u8 {
    if img[x][y] < 127 {
        return 0
    }
    return 1
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

    let mut pieces: [[[u8; 500];500];10] = [[[0; 500]; 500];10];

    println!("res={:?}", get_nearby(&pieces[0], 10, 10));

    //
	for x in 0..500 {
		for y in 0..500 {
            if x >= dims.0 || y >= dims.1 {
                continue;
            }
            let pix = im.get_pixel(x, y);
            if pix[0] > 127 {
                pieces[0][x as usize][y as usize] = 255;
            }
		}
	}

    for x in 0..500 {
		for y in 0..500 {
            if x >= dims.0 || y >= dims.1 {
                continue;
            }
            let v = pieces[0][x as usize][y as usize];
            let pix = image::Rgba([v,v,v,0]);
            im.put_pixel(x, y, pix);
		}
	}


    let ref mut fout = File::create(&Path::new(&format!("{}.png", file))).unwrap();

    // Write the contents of this image to the Writer in PNG format.
    let _ = im.save(fout, image::PNG).unwrap();
}
