extern crate hyper;

use hyper::server::{Server, Request, Response};
use std::io::{self, Write};
use std::io::Read;

static INDEX: &'static [u8] = br#"
<html>
<body>
<canvas id="canvas" width="500" height="500"></canvas>
<form action="/action_page_post.php" method="post" enctype="multipart/form-data">
<input type="file" name="filename" accept="image/gif, image/jpeg, image/png" id="uploadimage">
<input type="submit" value="Submit">
<script type="text/javascript">//<![CDATA[

function draw(ev) {
    console.log(ev);
    var ctx = document.getElementById('canvas').getContext('2d'),
        img = new Image(),
        f = document.getElementById("uploadimage").files[0],
        url = window.URL || window.webkitURL,
        src = url.createObjectURL(f);

    img.src = src;
    img.onload = function() {
        ctx.drawImage(img, 0, 0, 640, 480);
        url.revokeObjectURL(src);
    }
}

document.getElementById("uploadimage").addEventListener("change", draw, false)
//]]>

</script>
</form>
</body>
</html>"#;

fn handle_req(mut req: Request, res: Response) {

    // Print out all the headers first.
    for header in req.headers.iter() {
        println!("{}", header);
    }
    println!("");

    let mut buf = vec!();
    req.read_to_end(&mut buf).unwrap();
    println!("{:?}", buf);

    res.send(INDEX).unwrap();
}

fn main() {
    println!("starting server on 127.0.0.1:9090");
    Server::http("127.0.0.1:9090").unwrap().handle(handle_req).unwrap();
}
