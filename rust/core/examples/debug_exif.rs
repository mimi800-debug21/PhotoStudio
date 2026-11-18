use std::fs::File;
use std::io::BufReader;

fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exif.jpg");
    let f = File::open(&path).unwrap();
    let mut reader = BufReader::new(f);
    let mut r = kamadak_exif::Reader::new();
    r.continue_on_error(true);
    match r.read_from_container(&mut reader).or_else(|e| e.distill_partial_result(|_| {})) {
        Ok(exif) => {
            println!("fields: {}", exif.fields().len());
            for field in exif.fields() {
                println!("  {:?} ifd={:?} value={:?}", field.tag, field.ifd_num, field.value);
            }
            println!("buf len {}", exif.buf().len());
            let t = exif.get_field(kamadak_exif::Tag::JPEGInterchangeFormat, kamadak_exif::In::THUMBNAIL);
            println!("thumb tag: {:?}", t.map(|f| &f.value));
        }
        Err(e) => println!("ERR: {e:?}"),
    }
}
