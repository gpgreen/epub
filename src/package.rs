use crate::io::BufReader;
use crate::EPubError;
use alloc::{boxed::Box, fmt::Debug, string::String, vec::Vec};
use fatfs::{File, IoBase, OemCpConverter, Read, ReadWriteSeek, Seek, SeekFrom, TimeProvider};
use log::info;
use xml;

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
*/

pub struct Package {
    unique_identifer: Box<String>,
    version: Box<String>,
    xml_lang: Option<Box<String>>,
    prefix: Option<Box<String>>,
    id: Option<Box<String>>,
    dir: Option<Box<String>>,
    metadata: Option<Metadata>,
    manifest: Option<Manifest>,
    spine: Option<Spine>,
}

impl Package {
    pub fn read<
        IO: ReadWriteSeek + Debug + IoBase<Error = IO>,
        TP: TimeProvider,
        OCC: OemCpConverter,
    >(
        mut file: File<IO, TP, OCC>,
    ) -> Result<Package, EPubError<IO>> {
        let file_len = file
            .seek(SeekFrom::End(0))
            .map_err(|e| EPubError::<IO>::IO(e))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| EPubError::<IO>::IO(e))?;
        let mut rdr = BufReader::new(file)?;
        let mut p = xml::Parser::new();
        let mut e = xml::ElementBuilder::new();
        let mut bytes_read = 0;
        let mut v = Vec::new();
        loop {
            let mut arr = [0u8; 512];
            let n = rdr.read(&mut arr)?;
            v.extend(&arr[..n]);
            bytes_read += n;
            if bytes_read as u64 == file_len {
                break;
            }
        }
        p.feed_str(&String::from_utf8(v).map_err(|e| EPubError::<IO>::FromUTF8(e))?);
        for event in p.filter_map(|x| e.handle_event(x)) {
            // println!("{:?}", event);
            match event {
                Ok(e) => info!("{}", e),
                Err(e) => info!("{}", e),
            }
        }
        Ok(Package {
            unique_identifer: Box::new(String::new()),
            version: Box::new(String::new()),
            xml_lang: None,
            prefix: None,
            id: None,
            dir: None,
            metadata: None,
            manifest: None,
            spine: None,
        })
    }
}

pub struct Metadata {
    /// dc:title element
    title: Box<String>,
    /// dc:creator
    creators: Vec<Box<String>>,
    /// dc:date
    date: Box<String>,
    source_id: Box<String>,
    /// package:unique-identifer
    identifier: Box<String>,
    coverage: Box<String>,
    format: Box<String>,
    language: Vec<Box<String>>,
    rights: Box<String>,
    publisher: Box<String>,
}

pub struct Manifest {}

pub struct Spine {}
