extern crate hyper;

use hyper::server::{Server, Request, Response};
use std::io::{self, Write};
use std::io::Read;

static INDEX: &'static [u8] = br#"
<html>
<body>
<form action="/action_page_post.php" method="post" enctype="multipart/form-data">
<input type="file" name="filename" accept="image/gif, image/jpeg, image/png">
<input type="submit" value="Submit">
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
