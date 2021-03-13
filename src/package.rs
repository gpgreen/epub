use crate::io::BufReader;
use crate::EPubError;
use alloc::{string::String, vec::Vec};
use fatfs::{FileSystem, OemCpConverter, ReadWriteSeek, Seek, SeekFrom, TimeProvider};
use log::{info, trace, warn};
use xml::{Event, Parser, StartTag};

/*
<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" xml:lang="en" unique-identifier="p9781718500457">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:title>The Rust Programming Language</dc:title>
<dc:creator>Steve Klabnik</dc:creator>
<dc:creator>Carol Nichols</dc:creator>
<dc:date>2019</dc:date>
<meta property="dcterms:modified">2019-07-30T12:00:00Z</meta>
<dc:source id="src-id">urn:isbn:9781718500457</dc:source>
<dc:identifier id="p9781718500457">9781718500457</dc:identifier>
<dc:coverage>San Francisco</dc:coverage>
<dc:format>562 pages</dc:format>
<dc:type>Text</dc:type>
<dc:language>en</dc:language>
<dc:rights>All rights reserved.</dc:rights>
<dc:publisher>No Starch Press, Inc.</dc:publisher>
<meta name="cover" content="cover-image"/>
</metadata>
<manifest>
<item id="ncxtoc" media-type="application/x-dtbncx+xml" href="toc.ncx"/>
<item id="css" href="styles/9781718500457.css" media-type="text/css"/>
<item id="nav" href="xhtml/nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
<item id="font1" href="fonts/UbuntuMono-Regular.ttf" media-type="application/vnd.ms-opentype"/>
<item id="font2" href="fonts/UbuntuMono-Bold.ttf" media-type="application/vnd.ms-opentype"/>
<item id="font3" href="fonts/UbuntuMono-Italic.ttf" media-type="application/vnd.ms-opentype"/>
<item id="font4" href="fonts/UbuntuMono-BoldItalic.ttf" media-type="application/vnd.ms-opentype"/>
<item id="font5" href="fonts/JansonTextLTStd-Bold.otf" media-type="application/vnd.ms-opentype"/>
<item id="font6" href="fonts/JansonTextLTStd-BoldItalic.otf" media-type="application/vnd.ms-opentype"/>
<item id="font7" href="fonts/JansonTextLTStd-Italic.otf" media-type="application/vnd.ms-opentype"/>
<item id="font8" href="fonts/JansonTextLTStd-Roman.otf" media-type="application/vnd.ms-opentype"/>
<item id="font9" href="fonts/TradeGothicLTStd-Bold.otf" media-type="application/vnd.ms-opentype"/>
<item id="font10" href="fonts/TradeGothicLTStd-BoldObl.otf" media-type="application/vnd.ms-opentype"/>
<item id="font11" href="fonts/TradeGothicLTStd.otf" media-type="application/vnd.ms-opentype"/>
<item id="font12" href="fonts/TradeGothicLTStd-Obl.otf" media-type="application/vnd.ms-opentype"/>
<item id="font13" href="fonts/ARIALUNI.ttf" media-type="application/vnd.ms-opentype"/>
<item id="cover" href="xhtml/cover.xhtml" media-type="application/xhtml+xml"/>
<item id="title" href="xhtml/title.xhtml" media-type="application/xhtml+xml"/>
<item id="copy" href="xhtml/copy.xhtml" media-type="application/xhtml+xml"/>
<item id="author" href="xhtml/author.xhtml" media-type="application/xhtml+xml"/>
<item id="toc01" href="xhtml/toc01.xhtml" media-type="application/xhtml+xml"/>
<item id="toc" href="xhtml/toc.xhtml" media-type="application/xhtml+xml"/>
<item id="foreword" href="xhtml/foreword.xhtml" media-type="application/xhtml+xml"/>
<item id="preface" href="xhtml/preface.xhtml" media-type="application/xhtml+xml"/>
<item id="ack" href="xhtml/ack.xhtml" media-type="application/xhtml+xml"/>
<item id="intro" href="xhtml/intro.xhtml" media-type="application/xhtml+xml"/>
<item id="ch01" href="xhtml/ch01.xhtml" media-type="application/xhtml+xml"/>
<item id="ch02" href="xhtml/ch02.xhtml" media-type="application/xhtml+xml"/>
<item id="ch03" href="xhtml/ch03.xhtml" media-type="application/xhtml+xml"/>
<item id="ch04" href="xhtml/ch04.xhtml" media-type="application/xhtml+xml"/>
<item id="ch05" href="xhtml/ch05.xhtml" media-type="application/xhtml+xml"/>
<item id="ch06" href="xhtml/ch06.xhtml" media-type="application/xhtml+xml"/>
<item id="ch07" href="xhtml/ch07.xhtml" media-type="application/xhtml+xml"/>
<item id="ch08" href="xhtml/ch08.xhtml" media-type="application/xhtml+xml"/>
<item id="ch09" href="xhtml/ch09.xhtml" media-type="application/xhtml+xml"/>
<item id="ch10" href="xhtml/ch10.xhtml" media-type="application/xhtml+xml"/>
<item id="ch11" href="xhtml/ch11.xhtml" media-type="application/xhtml+xml"/>
<item id="ch12" href="xhtml/ch12.xhtml" media-type="application/xhtml+xml"/>
<item id="ch13" href="xhtml/ch13.xhtml" media-type="application/xhtml+xml"/>
<item id="ch14" href="xhtml/ch14.xhtml" media-type="application/xhtml+xml"/>
<item id="ch15" href="xhtml/ch15.xhtml" media-type="application/xhtml+xml"/>
<item id="ch16" href="xhtml/ch16.xhtml" media-type="application/xhtml+xml"/>
<item id="ch17" href="xhtml/ch17.xhtml" media-type="application/xhtml+xml"/>
<item id="ch18" href="xhtml/ch18.xhtml" media-type="application/xhtml+xml"/>
<item id="ch19" href="xhtml/ch19.xhtml" media-type="application/xhtml+xml"/>
<item id="ch20" href="xhtml/ch20.xhtml" media-type="application/xhtml+xml"/>
<item id="app01" href="xhtml/app01.xhtml" media-type="application/xhtml+xml"/>
<item id="app02" href="xhtml/app02.xhtml" media-type="application/xhtml+xml"/>
<item id="app03" href="xhtml/app03.xhtml" media-type="application/xhtml+xml"/>
<item id="app04" href="xhtml/app04.xhtml" media-type="application/xhtml+xml"/>
<item id="app05" href="xhtml/app05.xhtml" media-type="application/xhtml+xml"/>
<item id="index" href="xhtml/index.xhtml" media-type="application/xhtml+xml"/>
<item id="bm01" href="xhtml/bm01.xhtml" media-type="application/xhtml+xml"/>
<item id="bm02" href="xhtml/bm02.xhtml" media-type="application/xhtml+xml"/>
<item id="bm03" href="xhtml/bm03.xhtml" media-type="application/xhtml+xml"/>
<item id="cover-image" href="images/9781718500457.jpg" media-type="image/jpeg"/>
<item id="a04fig01" href="images/04fig01.jpg" media-type="image/jpeg"/>
<item id="a04fig02" href="images/04fig02.jpg" media-type="image/jpeg"/>
<item id="a04fig03" href="images/04fig03.jpg" media-type="image/jpeg"/>
<item id="a04fig03a" href="images/question.jpg" media-type="image/jpeg"/>
<item id="a04fig04" href="images/04fig04.jpg" media-type="image/jpeg"/>
<item id="a04fig05" href="images/04fig05.jpg" media-type="image/jpeg"/>
<item id="a04fig06" href="images/04fig06.jpg" media-type="image/jpeg"/>
<item id="a14fig01" href="images/14fig01.jpg" media-type="image/jpeg"/>
<item id="a14fig02" href="images/14fig02.jpg" media-type="image/jpeg"/>
<item id="a14fig03" href="images/14fig03.jpg" media-type="image/jpeg"/>
<item id="a14fig04" href="images/14fig04.jpg" media-type="image/jpeg"/>
<item id="a15fig01" href="images/15fig01.jpg" media-type="image/jpeg"/>
<item id="a15fig02" href="images/15fig02.jpg" media-type="image/jpeg"/>
<item id="a15fig03" href="images/15fig03.jpg" media-type="image/jpeg"/>
<item id="a15fig04" href="images/15fig04.jpg" media-type="image/jpeg"/>
<item id="a20fig01" href="images/20fig01.jpg" media-type="image/jpeg"/>
<item id="acommon" href="images/common.jpg" media-type="image/jpeg"/>
<item id="af0529-01" href="images/f0529-01.jpg" media-type="image/jpeg"/>
<item id="af0529-02" href="images/f0529-02.jpg" media-type="image/jpeg"/>
<item id="af0529-03" href="images/f0529-03.jpg" media-type="image/jpeg"/>
<item id="af0529-04" href="images/f0529-04.jpg" media-type="image/jpeg"/>
<item id="af0529-05" href="images/f0529-05.jpg" media-type="image/jpeg"/>
<item id="af0529-06" href="images/f0529-06.jpg" media-type="image/jpeg"/>
<item id="alogo" href="images/logo.jpg" media-type="image/jpeg"/>
<item id="alogo1" href="images/logo1.jpg" media-type="image/jpeg"/>
<item id="apub" href="images/pub.jpg" media-type="image/jpeg"/>
<item id="aaa1" href="images/backcover.jpg" media-type="image/jpeg"/>
<item id="aaa2" href="images/listing7-2.jpg" media-type="image/jpeg"/>
<item id="aaa3" href="images/pagae303.jpg" media-type="image/jpeg"/>
<item id="aaa4" href="images/pagae304.jpg" media-type="image/jpeg"/>
<item id="aaa5" href="images/page142.jpg" media-type="image/jpeg"/>
<item id="aaa6" href="images/page142a.jpg" media-type="image/jpeg"/>
<item id="aaa7" href="images/page142b.jpg" media-type="image/jpeg"/>
<item id="aaa8" href="images/page142c.jpg" media-type="image/jpeg"/>
<item id="aaa9" href="images/page143d.jpg" media-type="image/jpeg"/>
<item id="aaa10" href="images/page143f.jpg" media-type="image/jpeg"/>
<item id="aaa11" href="images/page143g.jpg" media-type="image/jpeg"/>
<item id="aaa12" href="images/page143h.jpg" media-type="image/jpeg"/>
<item id="aaa13" href="images/page40.jpg" media-type="image/jpeg"/>
<item id="aaa14" href="images/page_138_01.jpg" media-type="image/jpeg"/>
<item id="aaa15" href="images/page_138_02.jpg" media-type="image/jpeg"/>
<item id="aaa16" href="images/page_138_03.jpg" media-type="image/jpeg"/>
<item id="aaa17" href="images/page_138_04.jpg" media-type="image/jpeg"/>
<item id="aaa18" href="images/page_138_05.jpg" media-type="image/jpeg"/>
<item id="aaa19" href="images/page_138_06.jpg" media-type="image/jpeg"/>
<item id="aaa20" href="images/page_138_08.jpg" media-type="image/jpeg"/>
</manifest>
<spine toc="ncxtoc">
<itemref idref="cover"/>
<itemref idref="title"/>
<itemref idref="copy"/>
<itemref idref="author"/>
<itemref idref="toc01"/>
<itemref idref="toc"/>
<itemref idref="foreword"/>
<itemref idref="preface"/>
<itemref idref="ack"/>
<itemref idref="intro"/>
<itemref idref="ch01"/>
<itemref idref="ch02"/>
<itemref idref="ch03"/>
<itemref idref="ch04"/>
<itemref idref="ch05"/>
<itemref idref="ch06"/>
<itemref idref="ch07"/>
<itemref idref="ch08"/>
<itemref idref="ch09"/>
<itemref idref="ch10"/>
<itemref idref="ch11"/>
<itemref idref="ch12"/>
<itemref idref="ch13"/>
<itemref idref="ch14"/>
<itemref idref="ch15"/>
<itemref idref="ch16"/>
<itemref idref="ch17"/>
<itemref idref="ch18"/>
<itemref idref="ch19"/>
<itemref idref="ch20"/>
<itemref idref="app01"/>
<itemref idref="app02"/>
<itemref idref="app03"/>
<itemref idref="app04"/>
<itemref idref="app05"/>
<itemref idref="index"/>
<itemref idref="bm01"/>
<itemref idref="bm02"/>
<itemref idref="bm03"/>
</spine>
<guide>
<reference title="Cover Page" type="cover" href="xhtml/cover.xhtml"/>
<reference title="Title Page" type="text" href="xhtml/title.xhtml"/>
<reference title="Contents in Detail" type="toc" href="xhtml/toc.xhtml"/>
</guide>
</package>
*/

/// Package from EPub file
#[derive(Debug)]
pub struct Package {
    unique_identifer: String,
    version: String,
    xml_lang: Option<String>,
    prefix: Option<String>,
    id: Option<String>,
    dir: Option<String>,
    metadata: Metadata,
    manifest: Manifest,
    spine: Spine,
}

impl Package {
    /// read the package data from the file
    pub fn read<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        opf_file_name: &str,
        _container_file_name: &str,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<Package, EPubError<IO>> {
        let root_dir = fs.root_dir();
        // open the file
        let mut opf_file = root_dir.open_file(&opf_file_name)?;
        let _file_len = opf_file.seek(SeekFrom::End(0))?;
        opf_file.seek(SeekFrom::Start(0))?;
        let mut rdr = BufReader::new(opf_file)?;
        let mut p = Parser::new();
        let lines = rdr.read_lines()?;
        let mut stack: Vec<Event> = Vec::new();
        let mut chars = String::new();
        let mut metadata = Metadata::new();
        let mut manifest = Manifest::new();
        let mut spine = Spine::new();
        let mut in_package = false;
        let mut in_metadata = false;
        let mut in_manifest = false;
        let mut in_spine = false;
        for ln in lines {
            p.feed_str(&ln);
            for event in &mut p {
                match event {
                    Ok(e) => match e {
                        Event::PI(s) => info!("PI({})", s),
                        Event::ElementStart(tag) => {
                            info!("Start({})", tag.name);
                            if tag.name == "metadata" {
                                in_metadata = true;
                            } else if tag.name == "package" {
                                in_package = true;
                            } else if tag.name == "manifest" {
                                in_manifest = true;
                            } else if tag.name == "spine" {
                                in_spine = true;
                            }
                            stack.push(Event::ElementStart(tag));
                        }
                        Event::ElementEnd(tag) => {
                            info!("End({})", tag.name);
                            if let Some(last) = stack.pop() {
                                match last {
                                    Event::ElementStart(start_tag) => {
                                        if tag.name == "metadata" {
                                            in_metadata = false;
                                        } else if tag.name == "package" {
                                            in_package = false;
                                        } else if tag.name == "manifest" {
                                            in_manifest = false;
                                        } else if tag.name == "spine" {
                                            in_spine = false;
                                        }
                                        if in_metadata {
                                            metadata.add_tag(&start_tag, &chars);
                                        } else if in_manifest {
                                            manifest.add_tag(&start_tag);
                                        } else if in_spine {
                                            spine.add_tag(&start_tag);
                                        } else {
                                            trace!(
                                                "completed '{}' with chars '{}'",
                                                tag.name,
                                                chars
                                            );
                                        }
                                        chars = String::new();
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
                        Event::CDATA(s) => info!("CDATA({})", s),
                        Event::Comment(s) => info!("Comment({})", s),
                    },
                    Err(e) => {
                        return Err(EPubError::XmlParseErr(e));
                    }
                }
            }
        }
        Ok(Package {
            unique_identifer: String::new(),
            version: String::new(),
            xml_lang: None,
            prefix: None,
            id: None,
            dir: None,
            metadata: metadata,
            manifest: manifest,
            spine: spine,
        })
    }
}

/// Meta tag entry from opf file
#[derive(Debug)]
pub struct Meta {
    /// name - property name
    name: String,
    content: String,
}

impl Meta {
    /// create a new meta entry from xml tag 'meta'
    pub fn new(tag: &StartTag, chars: &str) -> Meta {
        // opf3 version
        if let Some(prop) = tag.attributes.get(&(String::from("property"), None)) {
            Meta {
                name: String::from(prop),
                content: String::from(chars),
            }
        // or the opf2 version
        } else if let Some(name) = tag.attributes.get(&(String::from("name"), None)) {
            if let Some(content) = tag.attributes.get(&(String::from("content"), None)) {
                Meta {
                    name: String::from(name),
                    content: String::from(content),
                }
            } else {
                panic!();
            }
        } else {
            panic!();
        }
    }
}

/// Metadata section from opf file
#[derive(Debug)]
pub struct Metadata {
    /// package:unique-identifer
    identifier: String,
    /// dc:title element
    title: String,
    /// dc::language
    language: Vec<String>,
    contributor: Option<String>,
    coverage: Option<String>,
    /// dc:creator
    creator: Vec<String>,
    /// dc:date
    date: Option<String>,
    description: Option<String>,
    format: Option<String>,
    publisher: Option<String>,
    relation: Option<String>,
    rights: Option<String>,
    source: Option<String>,
    subject: Option<String>,
    metadata_type: Option<String>,
    meta_tags: Vec<Meta>,
}

impl Metadata {
    /// create a new Metadata instance
    pub fn new() -> Metadata {
        Metadata {
            identifier: String::new(),
            title: String::new(),
            language: Vec::new(),
            contributor: None,
            coverage: None,
            creator: Vec::new(),
            date: None,
            description: None,
            format: None,
            publisher: None,
            relation: None,
            rights: None,
            source: None,
            subject: None,
            metadata_type: None,
            meta_tags: Vec::new(),
        }
    }

    /// add entry to the Metadata from xml tag
    pub fn add_tag(&mut self, tag: &StartTag, chars: &str) {
        trace!("metadata: '{}' with chars '{}'", tag.name, chars);
        for ((key1, key2), val) in &tag.attributes {
            trace!("attribute '{}':'{:?}' is '{}'", key1, key2, val);
        }
        if tag.name == "identifier" {
            // TODO, get attribute id
            self.identifier += chars;
        } else if tag.name == "title" {
            // has optional attributes dir,id,xml:lang
            self.title += chars;
        } else if tag.name == "language" {
            // has optional attributes id
            self.language.push(String::from(chars));
        } else if tag.name == "coverage" {
            self.coverage = Some(String::from(chars));
        } else if tag.name == "creator" {
            self.creator.push(String::from(chars));
        } else if tag.name == "date" {
            self.date = Some(String::from(chars));
        } else if tag.name == "description" {
            self.description = Some(String::from(chars));
        } else if tag.name == "format" {
            self.format = Some(String::from(chars));
        } else if tag.name == "publisher" {
            self.publisher = Some(String::from(chars));
        } else if tag.name == "relation" {
            self.relation = Some(String::from(chars));
        } else if tag.name == "rights" {
            self.rights = Some(String::from(chars));
        } else if tag.name == "source" {
            self.source = Some(String::from(chars));
        } else if tag.name == "subject" {
            self.subject = Some(String::from(chars));
        } else if tag.name == "type" {
            self.metadata_type = Some(String::from(chars));
        } else if tag.name == "meta" {
            self.meta_tags.push(Meta::new(tag, chars));
        } else {
            warn!("Metadata unknown tag name: '{}'", tag.name);
        }
    }
}

/// Manifest section of opf file
#[derive(Debug)]
pub struct Manifest {
    items: Vec<Item>,
}

impl Manifest {
    /// create a new manifest
    pub fn new() -> Manifest {
        Manifest { items: Vec::new() }
    }

    /// add an item tag instance to the manifest
    pub fn add_tag(&mut self, tag: &StartTag) {
        self.items.push(Item::new(tag))
    }
}

/// item tag of opf file
#[derive(Debug)]
pub struct Item {
    id: String,
    href: String,
    media_type: String,
}

impl Item {
    /// create a new item from the item tag
    pub fn new(tag: &StartTag) -> Item {
        if let Some(id) = tag.attributes.get(&(String::from("id"), None)) {
            if let Some(href) = tag.attributes.get(&(String::from("href"), None)) {
                if let Some(mtype) = tag.attributes.get(&(String::from("media-type"), None)) {
                    trace!("item {} ref='{}' m='{}'", id, href, mtype);
                    Item {
                        id: String::from(id),
                        href: String::from(href),
                        media_type: String::from(mtype),
                    }
                } else {
                    panic!();
                }
            } else {
                panic!();
            }
        } else {
            panic!();
        }
    }
}

/// spine section from opf file
#[derive(Debug)]
pub struct Spine {
    itemrefs: Vec<ItemRef>,
}

impl Spine {
    /// create a new spine
    pub fn new() -> Spine {
        Spine {
            itemrefs: Vec::new(),
        }
    }

    /// add an itemref tag instance to the spine
    pub fn add_tag(&mut self, tag: &StartTag) {
        self.itemrefs.push(ItemRef::new(tag))
    }
}

/// itemref tag of opf file
#[derive(Debug)]
pub struct ItemRef {
    idref: String,
}

impl ItemRef {
    /// create a new itemref from the itemref tag
    pub fn new(tag: &StartTag) -> ItemRef {
        if let Some(id) = tag.attributes.get(&(String::from("idref"), None)) {
            trace!("itemref {}", id);
            ItemRef {
                idref: String::from(id),
            }
        } else {
            panic!();
        }
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_event() {
        let mut p = xml::Parser::new();
        // feed data to be parsed
        p.feed_str("<a href");
        p.feed_str("='//example.com'/>");

        // get events for the fed data
        for event in p {
            match event.unwrap() {
                xml::Event::ElementStart(tag) => assert_eq!("a", tag.name),
                xml::Event::ElementEnd(tag) => assert_eq!("a", tag.name),
                _ => (),
            }
        }
    }
}
