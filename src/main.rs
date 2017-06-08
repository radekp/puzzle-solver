extern crate hyper;

use hyper::server::{Server, Request, Response};
use std::io::{self, Write};
use std::io::Read;

static INDEX: &'static [u8] = br##"


<head>
  <meta http-equiv="content-type" content="text/html; charset=UTF-8">
  <script type="text/javascript" src="//code.jquery.com/jquery-1.10.1.js"></script>
  <link rel="stylesheet" type="text/css" href="/css/result-light.css">
  <style type="text/css">
    body {
    background-color: ivory;
    }
    canvas {
    border:1px solid red;
    }
  </style>
  <title></title>
  <script type="text/javascript">//<![CDATA[
    $(window).load(function(){
    var canvas;
    var ctx;

    var canvasOffset;
    var offsetX;
    var offsetY;

    canvas = document.getElementById("canvas");
    ctx = canvas.getContext("2d");

    canvasOffset = $("#canvas").offset();
    offsetX = canvasOffset.left;
    offsetY = canvasOffset.top;

    $("#canvas").on('mousedown', function (e) {
        handleMouseDown(e);
    });

    function handleMouseDown(e) {
    	canvas.style.cursor = "crosshair";
    	var x = parseInt(e.clientX - offsetX);
    	var y = parseInt(e.clientY - offsetY);
        console.log("mousedown " + x + "," + y);
    }

    });//]]>

  </script>
</head>
<body>
  <p>Click once to set starting rectangle position</p>
  <p>Click again to set the ending position &amp; draw rectangle</p>
  <canvas style="cursor: default;" id="canvas" width="800" height="480"></canvas>
</body>
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
            ctx.drawImage(img, 0, 0, 800, 480);
            url.revokeObjectURL(src);
        }
    }

    document.getElementById("uploadimage").addEventListener("change", draw, false)
    //]]>

  </script>
</form>


"##;

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
    println!("starting server on 0.0.0.0:9090");
    Server::http("0.0.0.0:9090").unwrap().handle(handle_req).unwrap();
}
