//! the EPub Navigation Document
//! https://www.w3.org/publishing/epub32/epub-packages.html#sec-package-nav

use crate::{io::BufReader, package::Meta, EPubError};
use alloc::{string::String, vec::Vec};
use fatfs::{FileSystem, OemCpConverter, ReadWriteSeek, TimeProvider};
use log::{info, trace, warn};
use xml::{Event, Parser, StartTag};

/// TOC from EPub file
#[derive(Debug)]
pub struct Toc {
    pub meta_entries: Vec<Meta>,
    pub doc_title: String,
    pub nav_points: Vec<NavPoint>,
}

impl Toc {
    /// read the package data from the file
    pub fn read<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        toc_file_name: &str,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<Toc, EPubError<IO>> {
        let root_dir = fs.root_dir();
        // open the file
        let toc_file = root_dir.open_file(&toc_file_name)?;
        info!("Opened '{}'", toc_file_name);
        let mut rdr = BufReader::new(toc_file)?;
        let mut p = Parser::new();
        let lines = rdr.read_lines()?;
        let mut stack: Vec<Event> = Vec::new();
        let mut chars = String::new();
        let mut in_head = false;
        let mut in_doctitle = false;
        let mut in_navmap = false;
        let mut in_navpoint = false;
        let mut nav_point: Option<NavPoint> = None;
        let mut doc_title = String::new();
        let mut nav_points: Vec<NavPoint> = Vec::new();
        let mut meta_entries: Vec<Meta> = Vec::new();
        for ln in lines {
            p.feed_str(&ln);
            for event in &mut p {
                match event {
                    Ok(e) => match e {
                        Event::PI(s) => info!("PI({})", s),
                        Event::ElementStart(tag) => {
                            trace!("Start({})", tag.name);
                            if tag.name == "head" {
                                in_head = true;
                            } else if tag.name == "navMap" {
                                in_navmap = true;
                            } else if tag.name == "navPoint" && in_navmap {
                                in_navpoint = true;
                                nav_point = Some(NavPoint::new(&tag)?);
                            } else if tag.name == "docTitle" {
                                in_doctitle = true;
                            }
                            stack.push(Event::ElementStart(tag));
                            chars = String::new();
                        }
                        Event::ElementEnd(tag) => {
                            trace!("End({})", tag.name);
                            if let Some(last) = stack.pop() {
                                match last {
                                    Event::ElementStart(start_tag) => {
                                        if tag.name == "head" {
                                            in_head = false;
                                        } else if tag.name == "navMap" {
                                            in_navmap = false;
                                        } else if tag.name == "navPoint" {
                                            in_navpoint = false;
                                            if let Some(np) = nav_point {
                                                trace!("Adding navpoint: {:?}", np);
                                                nav_points.push(np);
                                                nav_point = None;
                                            }
                                        } else if tag.name == "docTitle" {
                                            in_doctitle = false;
                                        } else if tag.name == "text" {
                                            if in_navpoint {
                                                if let Some(mut np) = nav_point {
                                                    np.add_label(&chars);
                                                    nav_point = Some(np);
                                                }
                                            } else if in_doctitle {
                                                doc_title += &chars;
                                            }
                                        } else if tag.name == "content" && in_navpoint {
                                            if let Some(mut np) = nav_point {
                                                np.add_content::<IO>(&start_tag)?;
                                                nav_point = Some(np);
                                            }
                                        } else if tag.name == "meta" && in_head {
                                            let m = Meta::new(&start_tag, &chars);
                                            meta_entries.push(m);
                                        }
                                        assert!(start_tag.name == tag.name);
                                    }
                                    _ => (),
                                }
                            }
                        }
                        Event::Characters(s) => {
                            trace!("Characters({})", s);
                            if s != "\n" && s != "\r\n" {
                                chars += &s;
                            }
                        }
                        Event::CDATA(s) => warn!("CDATA({})", s),
                        Event::Comment(s) => warn!("Comment({})", s),
                    },
                    Err(e) => return Err(EPubError::XmlParseErr(e)),
                }
            }
        }
        info!("Finished parsing '{}'", toc_file_name);
        Ok(Toc {
            meta_entries,
            doc_title,
            nav_points,
        })
    }
}

/// NavPoint from EPub file
#[derive(Debug, Clone)]
pub struct NavPoint {
    pub id: String,
    pub play_order: u32,
    pub label: String,
    pub content: String,
}

impl NavPoint {
    pub fn new<IO: ReadWriteSeek>(tag: &StartTag) -> Result<NavPoint, EPubError<IO>> {
        match tag.attributes.get(&(String::from("id"), None)) {
            Some(id_val) => match tag.attributes.get(&(String::from("playOrder"), None)) {
                Some(order_val) => Ok(NavPoint {
                    id: String::from(id_val),
                    play_order: order_val.parse::<u32>().unwrap(),
                    label: String::new(),
                    content: String::new(),
                }),
                None => Err(EPubError::InvalidXml),
            },
            None => Err(EPubError::InvalidXml),
        }
    }

    pub fn add_label(&mut self, label: &str) {
        self.label += label;
    }

    pub fn add_content<IO: ReadWriteSeek>(&mut self, tag: &StartTag) -> Result<(), EPubError<IO>> {
        if let Some(content) = tag.attributes.get(&(String::from("src"), None)) {
            self.content += content;
            Ok(())
        } else {
            Err(EPubError::InvalidXml)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fatfs::StdIoWrapper;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn test_navpoint() {
        init();
        let mut p = Parser::new();
        // feed data to be parsed
        p.feed_str(
            "<navPoint id=\"i1\" playOrder=\"1\">\n<navLabel>\n<text>Cover Page</text>\n</navLabel>\n<content src=\"xhtml/cover.xhtml\"/>\n</navPoint>\n"
        );
        let mut navp: Option<NavPoint> = None;
        let mut s = String::new();
        // get events for the fed data
        for event in p {
            match event.unwrap() {
                Event::ElementStart(tag) => {
                    for ((key1, key2), val) in &tag.attributes {
                        trace!("attribute '{}:{:?}' is '{}'", key1, key2, val);
                    }
                    if tag.name == "navPoint" {
                        match NavPoint::new::<StdIoWrapper<std::fs::File>>(&tag) {
                            Ok(n) => {
                                navp = Some(n);
                            }
                            Err(_) => panic!(),
                        }
                    } else if tag.name == "content" {
                        if let Some(mut n) = navp {
                            match n.add_content::<StdIoWrapper<std::fs::File>>(&tag) {
                                Ok(_) => (),
                                Err(_) => panic!(),
                            }
                            navp = Some(n);
                        }
                    }
                    s = String::new();
                }
                Event::ElementEnd(tag) => {
                    if tag.name == "text" {
                        if let Some(mut n) = navp {
                            n.add_label(&s);
                            navp = Some(n);
                        }
                    }
                }
                Event::Characters(ch) => {
                    if ch != "\n" {
                        s += &ch;
                    }
                }
                _ => (),
            }
        }
        if let Some(n) = navp {
            assert_eq!(n.id, "i1");
            assert_eq!(n.play_order, 1);
            assert_eq!(n.label, "Cover Page");
            assert_eq!(n.content, "xhtml/cover.xhtml");
        } else {
            panic!();
        }
    }
}

/*
<?xml version="1.0"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1" xml:lang="en">
<head>
<meta name="dtb:uid" content="9781718500457"/>
<meta name="dtb:depth" content="1"/>
<meta name="dtb:totalPageCount" content="0"/>
<meta name="dtb:maxPageNumber" content="0"/>
</head>
<docTitle>
<text>The Rust Programming Language</text>
</docTitle>
<navMap>
<navPoint id="i1" playOrder="1">
<navLabel>
<text>Cover Page</text>
</navLabel>
<content src="xhtml/cover.xhtml"/>
</navPoint>
<navPoint id="i2" playOrder="2">
<navLabel>
<text>Title Page</text>
</navLabel>
<content src="xhtml/title.xhtml"/>
</navPoint>
...
</navMap>
</ncx>
*/
