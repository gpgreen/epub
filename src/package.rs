//! the EPub Package Document
//! https://www.w3.org/publishing/epub32/epub-packages.html#sec-package-doc

use crate::io;
use crate::io::BufReader;
use crate::EPubError;
use alloc::{string::String, vec::Vec};
use fatfs::{FileSystem, OemCpConverter, ReadWriteSeek, Seek, SeekFrom, TimeProvider};
use log::{info, trace, warn};
use xml::{Event, Parser, StartTag};

/// Package from EPub file
#[derive(Debug)]
pub struct Package {
    /// attribute `unique-identifier`
    pub unique_identifer: String,
    /// attribute `version`
    pub version: String,
    /// attribute `xml:lang`
    pub xml_lang: Option<String>,
    //prefix: Option<String>,
    //id: Option<String>,
    //dir: Option<String>,
    /// `metadata` section
    pub metadata: Metadata,
    /// `manifest` section
    pub manifest: Manifest,
    /// `spine` section
    pub spine: Spine,
    /// the base directory
    ///
    /// This is where the bulk of the books' files reside
    pub base_dir: String,
}

impl Package {
    /// read the package data from the file
    pub fn read<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        opf_file_name: &str,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<Package, EPubError<IO>> {
        // get the leading directories from the file name
        let base_name = io::basename_and_ext(opf_file_name);
        let mut split = opf_file_name.split(&base_name.0);
        let base_dir = String::from(split.next().unwrap_or(""));
        // open the file
        let root_dir = fs.root_dir();
        let mut opf_file = root_dir.open_file(&opf_file_name)?;
        info!("Opened '{}' package", opf_file_name);
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
        let mut in_metadata = false;
        let mut in_manifest = false;
        let mut in_spine = false;
        // the attributes on package
        let mut package_uid: Option<String> = None;
        let mut version: Option<String> = None;
        let mut xml_lang: Option<String> = None;
        for ln in lines {
            p.feed_str(&ln);
            for event in &mut p {
                match event {
                    Ok(e) => match e {
                        Event::PI(s) => info!("PI({})", s),
                        Event::ElementStart(tag) => {
                            trace!("Start({})", tag.name);
                            if tag.name == "metadata" {
                                in_metadata = true;
                            } else if tag.name == "manifest" {
                                in_manifest = true;
                            } else if tag.name == "spine" {
                                in_spine = true;
                                spine.add_tag(&tag);
                            }
                            stack.push(Event::ElementStart(tag));
                            chars = String::new();
                        }
                        Event::ElementEnd(tag) => {
                            trace!("End({})", tag.name);
                            if let Some(last) = stack.pop() {
                                match last {
                                    Event::ElementStart(start_tag) => {
                                        if tag.name == "metadata" {
                                            in_metadata = false;
                                        } else if tag.name == "package" {
                                            let (a1, a2, a3) =
                                                Package::collect_attributes(&start_tag);
                                            package_uid = Some(a1);
                                            version = Some(a2);
                                            xml_lang = a3;
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
        info!("Finished parsing '{}' package", opf_file_name);
        if let Some(uid) = package_uid {
            if let Some(ver) = version {
                Ok(Package {
                    unique_identifer: uid,
                    version: ver,
                    xml_lang: xml_lang,
                    //prefix: None,
                    //id: None,
                    //dir: None,
                    metadata: metadata,
                    manifest: manifest,
                    spine: spine,
                    base_dir: base_dir,
                })
            } else {
                panic!();
            }
        } else {
            panic!();
        }
    }

    fn collect_attributes(start_tag: &StartTag) -> (String, String, Option<String>) {
        let mut uidstr = String::new();
        let mut verstr = String::new();
        let mut langstr: Option<String> = None;
        if let Some(uid) = start_tag
            .attributes
            .get(&(String::from("unique-identifier"), None))
        {
            uidstr += uid;
        }
        if let Some(ver) = start_tag.attributes.get(&(String::from("version"), None)) {
            verstr += ver;
        }
        // optional
        if let Some(lang) = start_tag.attributes.get(&(
            String::from("lang"),
            Some(String::from("http://www.w3.org/XML/1998/namespace")),
        )) {
            langstr = Some(String::from(lang));
        }
        (uidstr, verstr, langstr)
    }
}

/// Meta tag entry from opf file
#[derive(Debug)]
pub struct Meta {
    /// name - property name
    pub name: String,
    pub content: String,
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
    identifier: Identifier,
    /// dc:title element
    title: String,
    /// dc::language
    language: Vec<String>,
    /// dc::contributor
    contributor: Option<String>,
    /// dc::coverage
    coverage: Option<String>,
    /// dc:creator
    creator: Vec<String>,
    /// dc:date
    date: Option<String>,
    /// dc::description
    description: Option<String>,
    /// dc::format
    format: Option<String>,
    /// dc::publisher
    publisher: Option<String>,
    /// dc::relation
    relation: Option<String>,
    /// dc::rights
    rights: Option<String>,
    /// dc::source
    source: Option<String>,
    /// dc::subject
    subject: Option<String>,
    /// dc::type
    metadata_type: Option<String>,
    /// list of `meta` tags
    meta_tags: Vec<Meta>,
}

impl Metadata {
    /// create a new Metadata instance
    pub fn new() -> Metadata {
        Metadata {
            identifier: Identifier::new(),
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
            trace!("attribute '{}:{:?}' is '{}'", key1, key2, val);
        }
        if tag.name == "identifier" {
            self.identifier.add_tag(tag, chars);
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

/// dc::identifier
#[derive(Debug)]
pub struct Identifier {
    id: String,
    text: String,
}

impl Identifier {
    pub fn new() -> Identifier {
        Identifier {
            id: String::new(),
            text: String::new(),
        }
    }

    pub fn add_tag(&mut self, tag: &StartTag, chars: &str) {
        if let Some(id) = tag.attributes.get(&(String::from("id"), None)) {
            self.id += id;
            self.text += chars;
        } else {
            panic!();
        }
    }
}

/// Manifest section of opf file
#[derive(Debug)]
pub struct Manifest {
    pub items: Vec<Item>,
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
    pub id: String,
    pub href: String,
    pub media_type: String,
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
    pub itemrefs: Vec<ItemRef>,
    pub toc: String,
}

impl Spine {
    /// create a new spine
    pub fn new() -> Spine {
        Spine {
            itemrefs: Vec::new(),
            toc: String::new(),
        }
    }

    /// add an itemref tag instance to the spine
    pub fn add_tag(&mut self, tag: &StartTag) {
        if tag.name == "spine" {
            if let Some(toc) = tag.attributes.get(&(String::from("toc"), None)) {
                self.toc += toc;
            } else {
                panic!();
            }
        } else {
            self.itemrefs.push(ItemRef::new(tag))
        }
    }
}

/// itemref tag of opf file
#[derive(Debug)]
pub struct ItemRef {
    pub idref: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn test_package_attributes() {
        init();
        let mut p = xml::Parser::new();
        // feed data to be parsed
        p.feed_str(
            "<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" xml:lang=\"en\" unique-identifier=\"p9781718500457\">"
        );
        // get events for the fed data
        for event in p {
            match event.unwrap() {
                xml::Event::ElementStart(tag) => {
                    for ((key1, key2), val) in &tag.attributes {
                        info!("attribute '{}:{:?}' is '{}'", key1, key2, val);
                    }
                    let (s1, s2, s3) = Package::collect_attributes(&tag);
                    assert_eq!(s1, "p9781718500457");
                    assert_eq!(s2, "3.0");
                    assert_eq!(s3.unwrap(), "en");
                }
                _ => (),
            }
        }
    }

    #[test]
    fn test_manifestitem() {
        let mut p = xml::Parser::new();
        // feed data to be parsed
        p.feed_str(
            "<item id=\"ncxtoc\" media-type=\"application/x-dtbncx+xml\" href=\"toc.ncx\"/>",
        );
        let mut manifest = Manifest::new();
        // get events for the fed data
        for event in p {
            match event.unwrap() {
                xml::Event::ElementStart(tag) => manifest.add_tag(&tag),
                _ => (),
            }
        }
        assert_eq!(manifest.items.len(), 1);
    }

    #[test]
    fn test_item() {
        let mut p = xml::Parser::new();
        // feed data to be parsed
        p.feed_str(
            "<item id=\"ncxtoc\" media-type=\"application/x-dtbncx+xml\" href=\"toc.ncx\"/>",
        );

        // get events for the fed data
        for event in p {
            match event.unwrap() {
                xml::Event::ElementStart(tag) => {
                    let itm = Item::new(&tag);
                    assert_eq!(itm.id, "ncxtoc");
                    assert_eq!(itm.media_type, "application/x-dtbncx+xml");
                    assert_eq!(itm.href, "toc.ncx");
                }
                _ => (),
            }
        }
    }

    #[test]
    #[should_panic]
    fn test_baditem() {
        let mut p = xml::Parser::new();

        p.feed_str(
            "<item id=\"ncxtoc\" media-types=\"application/x-dtbncx+xml\" href=\"toc.ncx\"/>",
        );
        // get events for the fed data
        for event in p {
            match event.unwrap() {
                xml::Event::ElementStart(tag) => {
                    let _itm = Item::new(&tag);
                }
                _ => (),
            }
        }
    }

    #[test]
    fn test_spineitemref() {
        let mut p = xml::Parser::new();
        // feed data to be parsed
        p.feed_str("<itemref idref=\"copy\"/>");
        let mut spine = Spine::new();
        // get events for the fed data
        for event in p {
            match event.unwrap() {
                xml::Event::ElementStart(tag) => spine.add_tag(&tag),
                _ => (),
            }
        }
        assert_eq!(spine.itemrefs.len(), 1);
    }

    #[test]
    fn test_itemref() {
        let mut p = xml::Parser::new();
        // feed data to be parsed
        p.feed_str("<itemref idref=\"copy\"/>");

        // get events for the fed data
        for event in p {
            match event.unwrap() {
                xml::Event::ElementStart(tag) => {
                    let itmref = ItemRef::new(&tag);
                    assert_eq!(itmref.idref, "copy");
                }
                _ => (),
            }
        }
    }

    #[test]
    #[should_panic]
    fn test_baditemref() {
        let mut p = xml::Parser::new();
        // feed data to be parsed
        p.feed_str("<itemref idrefs=\"copy\"/>");

        // get events for the fed data
        for event in p {
            match event.unwrap() {
                xml::Event::ElementStart(tag) => {
                    let _itmref = ItemRef::new(&tag);
                }
                _ => (),
            }
        }
    }
}
