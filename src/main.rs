use std::error::Error;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::path::PathBuf;
use std::string::String;
use std::string::ToString;

use clap::{Parser, Subcommand};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use hex;
use ini::Ini;
use sha1::{Digest, Sha1};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        #[arg(default_value_t=(&".").to_string())]
        path: String,
    },
    CatFile {
        object_type: String,
        object: String,
    }
}

struct GitRepository {
    worktree: String,
    gitdir: String,
}

impl GitRepository {
    fn new(path: &str, force: bool) -> Result<GitRepository, Box<dyn Error>> {
        let worktree = path.to_string();
        let gitdir = format!("{}/.git", worktree);

        if !(force || Path::new(&gitdir).is_dir()) {
            return Err(From::from(format!("Not a git repository {}", path)));
        };

        Ok(GitRepository { worktree, gitdir })
    }
}

enum GitObject {
    Commit(GitCommit),
    Tree(GitTree),
    Tag(GitTag),
    Blob(GitBlob),
}

impl fmt::Display for GitObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GitObject::Commit(_) => write!(f, "commit"),
            GitObject::Tree(_) => write!(f, "tree"),
            GitObject::Tag(_) => write!(f, "tag"),
            GitObject::Blob(_) => write!(f, "blob"),
        }
    }
}

impl GitObject {
    fn new(data: Vec<u8>, object_type: &str) -> Result<Self, Box<dyn Error>> {
        match object_type {
            "commit" => Ok(GitObject::Commit(GitCommit::new(data))),
            "tree" => Ok(GitObject::Tree(GitTree::new(data))),
            "tag" => Ok(GitObject::Tag(GitTag::new(data))),
            "blob" => Ok(GitObject::Blob(GitBlob::new(data))),
            _ => Err(From::from(format!("Unknown type {}", object_type))),
        }
    }

    fn serialize(&self) -> Result<&Vec<u8>, Box<dyn Error>> {
        match self {
            GitObject::Commit(c) => Ok(c.serialize()),
            GitObject::Tree(t) => Ok(t.serialize()),
            GitObject::Tag(t) => Ok(t.serialize()),
            GitObject::Blob(b) => Ok(b.serialize()),
            _ => Err(From::from(format!("Unknown type {}", self))),
        }
    }

    fn deserialize(&self) {
        self.deserialize();
    }
}

struct GitCommit {
    data: Vec<u8>,
}

struct GitTree {
    data: Vec<u8>,
}

struct GitTag {
    data: Vec<u8>,
}

struct GitBlob {
    data: Vec<u8>,
}

impl GitObjectBehavior for GitCommit {
    fn new(data: Vec<u8>) -> Self {
        GitCommit { data }
    }

    fn serialize(&self) -> &Vec<u8> {
        todo!()
    }

    fn deserialize(data: &[u8]) -> Result<Self, String>
    where
        Self: Sized,
    {
        todo!()
    }
}

impl GitObjectBehavior for GitTree {
    fn new(data: Vec<u8>) -> Self {
        GitTree { data }
    }

    fn serialize(&self) -> &Vec<u8> {
        &self.data
    }

    fn deserialize(data: &[u8]) -> Result<Self, String>
    where
        Self: Sized,
    {
        todo!()
    }
}

impl GitObjectBehavior for GitTag {
    fn new(data: Vec<u8>) -> Self {
        GitTag { data }
    }

    fn serialize(&self) -> &Vec<u8> {
        todo!()
    }

    fn deserialize(data: &[u8]) -> Result<Self, String>
    where
        Self: Sized,
    {
        todo!()
    }
}

impl GitObjectBehavior for GitBlob {
    fn new(data: Vec<u8>) -> Self {
        GitBlob { data }
    }

    fn serialize(&self) -> &Vec<u8> {
        &self.data
    }

    fn deserialize(data: &[u8]) -> Result<Self, String>
    where
        Self: Sized,
    {
        todo!()
    }
}

trait GitObjectBehavior {
    fn new(data: Vec<u8>) -> Self
    where
        Self: Sized;

    fn serialize(&self) -> &Vec<u8>;

    fn deserialize(data: &[u8]) -> Result<Self, String>
    where
        Self: Sized;
}

fn read_object(repo: &GitRepository, sha: &str) -> Result<GitObject, Box<dyn Error>> {
    let path = repo_file(
        &repo,
        &format!("objects/{0}/{1}", &sha[0..=1], &sha[2..]).as_str(),
    );

    assert!(path.is_file());

    let file = File::open(path)?;

    let mut decoder = ZlibDecoder::new(file);
    let mut decompressed_data: Vec<u8> = Vec::new();

    let file_length: usize = decoder.read_to_end(&mut decompressed_data)?;

    let ascii_space = decompressed_data.iter().position(|&b| b == b' ').unwrap();
    let object_type: &[u8] = &decompressed_data[0..ascii_space];
    let object_type_string: String = String::from_utf8(object_type.to_vec())?;

    let null_byte: usize = decompressed_data.iter().position(|&b| b == b'\0').unwrap();
    let size: &str = std::str::from_utf8(&decompressed_data[ascii_space + 1..null_byte]).unwrap();
    let size: usize = size.parse::<usize>()?;

    if size != file_length - null_byte - 1 {
        return Err(From::from(format!("Malformed object {0}: bad length", sha)));
    }

    let object_content = decompressed_data[null_byte + 1..].to_vec();

    GitObject::new(object_content, object_type_string.as_str())
}

fn write_object(repo: &GitRepository, obj: &GitObject) -> String {
    let data = obj.serialize().unwrap();
    let data_slice = &data;

    // Create the header
    let header = format!("{} {}{}", obj.to_string(), data_slice.len(), '\0');

    // Concatenate the header and data
    let file_content = [header.as_bytes(), data_slice].concat();
    let mut hasher = Sha1::new();
    hasher.update(&file_content);
    let sha1_hash = hasher.finalize().to_vec();
    let sha1_hex = hex::encode(sha1_hash);

    let path = repo_file(
        &repo,
        &format!("objects/{0}1/{1}", &sha1_hex[0..=1], &sha1_hex[2..]).as_str(),
    );
    if !path.exists() {
        let file = File::create(path).unwrap();
        let mut encoder = ZlibEncoder::new(file, flate2::Compression::best());
        encoder.write_all(&file_content).unwrap();
    }

    return sha1_hex;
}

fn repo_path(repo: &GitRepository, path: &str) -> PathBuf {
    Path::new(&repo.gitdir).join(&path).to_path_buf()
}

fn repo_file(repo: &GitRepository, path: &str) -> PathBuf {
    let path = repo_path(&repo, &path);
    let dirs = path.parent().unwrap();
    let _ = repo_dir(&repo, &dirs.to_str().unwrap());
    path
}

fn repo_dir(repo: &GitRepository, path: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = repo_path(&repo, &path);
    if path.exists() {
        if path.is_dir() {
            return Ok(path.clone());
        } else {
            return Err(From::from(format!("Not a directory {path:?}")));
        }
    }

    fs::create_dir_all(&path).expect("Couldn't create directories {path:?}");
    Ok(path.to_path_buf())
}

fn repo_find(path: &str) -> Result<GitRepository, Box<dyn Error>> {
    let current_path = fs::canonicalize(Path::new(path)).unwrap();

    if current_path.join(".git").is_dir() {
            return GitRepository::new(current_path.to_str().unwrap(), false);
        }

    let current_path_ref: &Path = current_path.as_path();

    let mut current_dir = current_path_ref;

    while let Some(parent) = current_dir.parent() {
        if parent.join(".git").is_dir() {
            return GitRepository::new(parent.to_str().unwrap(), false);
        }
        current_dir = parent;
    }

    Err(From::from(format!("No .git repository")))
}

fn is_dir_empty(dir_path: &str) -> bool {
    if let Ok(entries) = fs::read_dir(dir_path) {
        for _ in entries {
            return false;
        }
    }
    true
}

fn repo_create(path: &str) -> Result<GitRepository, Box<dyn Error>> {
    let repo = GitRepository::new(&path, true).unwrap();

    let worktree = Path::new(&repo.worktree);
    let gitdir = Path::new(&repo.gitdir);
    if worktree.exists() {
        if !worktree.is_dir() {
            return Err(From::from("{worktree:?} is not a directory!"));
        }

        if gitdir.exists() && !is_dir_empty(&repo.gitdir) {
            return Err(From::from("{gitdir} is not empty!"));
        }
    } else {
        let _ = fs::create_dir_all(&worktree);
    }

    assert!(repo_dir(&repo, "branches").is_ok());
    assert!(repo_dir(&repo, "objects").is_ok());
    assert!(repo_dir(&repo, "refs/tags").is_ok());
    assert!(repo_dir(&repo, "refs/heads").is_ok());

    let description = "Unnamed repository; edit this file 'description' to name the repository.\n";
    let description_file = repo_file(&repo, "description");
    File::create(&description_file)?.write(description.as_bytes())?;

    repo_default_config().write_to_file(repo_file(&repo, "config"))?;

    Ok(repo)
}

fn cat_file(repo: &GitRepository, obj: &String) -> Result<(), Box<dyn Error>> {
    let git_obj = read_object(&repo, obj)?;
    let serialized_data = git_obj.serialize()?;

    // Get the mutable stdout handle
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Write the serialized data to stdout
    handle.write_all(&serialized_data)?;

    Ok(())
}

fn repo_default_config() -> Ini {
    let mut config = Ini::new();

    config
        .with_section(Some("core"))
        .set("repositoryformatversion", "0")
        .set("filemode", "false")
        .set("bare", "false");

    config
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Init { path }) => {
            let _ = repo_create(path).unwrap();
        }
        Some(Commands::CatFile { object_type, object} ) => {
            let repo = repo_find(".").unwrap();
            let _ = cat_file(&repo, object);
        }
        None => {
            let repo = repo_find("target").unwrap();
            let git_obj: GitObject =
                read_object(&repo, &"4089f12ac270e114bdff71ba7a01ea86fe2f4319").unwrap();
            write_object(&repo, &git_obj);
            match git_obj {
                GitObject::Commit(_) => {}
                GitObject::Tree(t) => {}
                GitObject::Tag(_) => {}
                GitObject::Blob(_) => {}
            }
            println!("Unknown command");
        }
    }
}
