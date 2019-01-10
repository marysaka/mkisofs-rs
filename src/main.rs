use std;
use std::env;
use std::io;
use std::fs;
use std::fs::DirEntry;
use std::fs::Metadata;
use std::path::PathBuf;

#[derive(Debug)]
struct FileEntry
{
    pub path: PathBuf,
    pub size: usize
}

#[derive(Debug)]
struct DirectoryEntry
{
    pub path: PathBuf,
    pub dir_childs : Vec<DirectoryEntry>,
    pub files_childs : Vec<FileEntry>
}

fn main() {
    let mut args = env::args();
    let executable_path = args.next().unwrap();

    match args.len()
    {
        2 => {
            let output_path = args.next().unwrap();
            let input_directory = args.next().unwrap();
            println!("Output file {}", output_path);
            println!("Input directory {}", input_directory);
            create_grub_iso(output_path, input_directory);
        },
        _ => println!("Usage: {} out.iso input_directory", executable_path)
    }
}

fn construct_directory(path : PathBuf) -> std::io::Result<DirectoryEntry>
{
    let dir_path = path.clone();
    let mut dir_childs : Vec<DirectoryEntry> = Vec::new();
    let mut files_childs : Vec<FileEntry> = Vec::new();

    for entry_res in fs::read_dir(path)? {
        let entry : DirEntry = entry_res?;
        let entry_meta : Metadata = entry.metadata()?;
        println!("{:?}", entry.path());
        if entry_meta.is_dir() {
            dir_childs.push(construct_directory(entry.path())?);
        } else if entry_meta.is_file() {
            files_childs.push(FileEntry { path: entry.path(), size: entry_meta.len() as usize})
        }
    }
    return Ok(DirectoryEntry {path: dir_path, dir_childs, files_childs});
}

fn create_grub_iso(output_path : String, input_directory : String)
{
    let tree = construct_directory(PathBuf::from(input_directory)).unwrap();
    println!("{:?}", tree);
    // TODO everything
}