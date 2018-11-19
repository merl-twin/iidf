#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate snap;

use clap::Arg;
use std::io::{BufReader,BufRead,BufWriter,Read,Write};
use std::collections::{BTreeMap,BTreeSet};
use std::fs::File;
use std::path::PathBuf;

#[derive(Debug)]
enum Error {
    Arg(clap::Error),
    Json(serde_json::Error),
    FileOpen(std::io::Error),
    FileCreate(std::io::Error),
    Rename(std::io::Error),
    Read(std::io::Error),
}

#[derive(Debug,Serialize,Deserialize,PartialEq,Eq,PartialOrd,Ord,Clone)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
enum Representation {
    Word {
        word: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        stem: Option<String>
    },
    Number { word: String },
    Alphanumeric { word: String },
    Emoji { word: String },
    Unicode { word: String },
    Hashtag { word: String },
    Mention { word: String },
    Url { word: String },
    BBCode {
        text: Vec<Representation>,
        data: Vec<Representation>,
    },
}

#[derive(Debug,Serialize,Deserialize)]
struct Doc {
    yauid: u64,
    words: Vec<Representation>,
}

#[derive(Debug,Serialize,Deserialize)]
struct IdfData {
    docs: u64,
    words: Vec<(Representation,u64)>,
}

#[derive(Debug)]
struct Idf {
    docs: u64,
    words: BTreeMap<Representation,u64>,
}
impl Idf {
    fn new() -> Idf {
        Idf {
            docs: 0,
            words: BTreeMap::new(),
        }
    }
    fn save_to<W: Write>(self, wrt: W) -> Result<(),Error> {
        let dt = IdfData {
            docs: self.docs,
            words: self.words.into_iter().collect(),
        };
        serde_json::to_writer(wrt,&dt).map_err(Error::Json)
    }
    fn load_from<R: Read>(rdr: R) -> Result<Idf,Error> {
        let dt: IdfData = serde_json::from_reader(rdr).map_err(Error::Json)?;
        Ok(Idf {
            docs: dt.docs,
            words: dt.words.into_iter().collect(),
        })
    }
    fn append(&mut self, data: Idf, min_df: u64) {
        self.docs += data.docs;
        for (k,v) in data.words {
            if v>=min_df {
                let mut cnt = self.words.entry(k).or_insert(0);
                *cnt += v;
            }
        }
    }
    fn words_count(&self) -> u64 {
        self.words.len() as u64
    }
    fn docs_count(&self) -> u64 {
        self.docs
    }
}

fn main() -> Result<(),Error> {
    //env_logger::init();
    let matches = app_from_crate!()
        .arg(Arg::with_name("iidf")
             .display_order(1)
             .short("i")
             .long("iidf")
             .value_name("IIDF")
             .help("idf data")
             .takes_value(true))
        .arg(Arg::with_name("file")
             .display_order(2)
             .short("f")
             .long("file")
             .value_name("FILE")
             .help("data chunk")
             .takes_value(true))
        .get_matches();


    let model = value_t!(matches, "iidf", String).map_err(Error::Arg)?;
    let file = value_t!(matches, "file", String).map_err(Error::Arg)?;
    
    let mut data = Idf::new();
    for row in BufReader::new(snap::Reader::new(BufReader::new(File::open(file).map_err(Error::FileOpen)?))).lines() {
        let js = row.map_err(Error::Read)?;
        let doc: Doc = serde_json::from_str(&js).map_err(Error::Json)?;
        //let doc_word_set = doc.words.into_iter().collect::<BTreeSet<_>>();
        for w in doc.words.into_iter().flat_map(|w| match w {
            Representation::BBCode { text, data: _ } => text.into_iter(),
            _ => vec![w].into_iter(),
        }) {
            let mut cnt = data.words.entry(w).or_insert(0);
            *cnt += 1;
        }
        data.docs += 1;
    }

    let mut iidf = if !PathBuf::from(&model).exists() {
        Idf::new()
    } else {
        Idf::load_from(snap::Reader::new(BufReader::new(File::open(&model).map_err(Error::FileOpen)?)))?
    };
    
    let min_df = 5;
    iidf.append(data,min_df);
    println!("Vocabulary: {} / {}",iidf.words_count(),iidf.docs_count());

    let pth = format!("{}.tmp",model);
    iidf.save_to(snap::Writer::new(BufWriter::new(File::create(&pth).map_err(Error::FileCreate)?)))?;

    std::fs::rename(pth,model).map_err(Error::Rename)
}
