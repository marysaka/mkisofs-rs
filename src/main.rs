extern crate byteorder;

use byteorder::{LittleEndian, BigEndian, ReadBytesExt, WriteBytesExt};
use std;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::fs::File;
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
            create_grub_iso(output_path, input_directory).unwrap();
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
    
    Ok(DirectoryEntry {path: dir_path, dir_childs, files_childs})
}

enum VolumeDescriptor
{
    Primary,
    Boot,
    Supplementary,
    End
}

impl VolumeDescriptor
{
    fn get_type_id(&self) -> u8
    {
        match self
        {
            Primary => 1,
            Boot => 2,
            End => 0xff
        }
    }

    fn write_volume_header<T>(&mut self, output_writter: &mut T) -> std::io::Result<()> where T: Write
    {
        let type_id = self.get_type_id();
        output_writter.write_u8(type_id)?;
        output_writter.write_all(b"CD001")?;
        output_writter.write_u8(0x1)?;
        Ok(())
    }

    fn write_volume<T>(&mut self, output_writter: &mut T) -> std::io::Result<()> where T: Write
    {
        let data : [u8; 2041] = [0; 2041];

        self.write_volume_header(output_writter)?;

        // todo data
        output_writter.write_all(&data)?;
        Ok(())
    }
}

fn generate_volume_descriptors() -> Vec<VolumeDescriptor>
{
    let mut res : Vec<VolumeDescriptor> = Vec::new();

    res.push(VolumeDescriptor::Primary);
    res.push(VolumeDescriptor::End);

    res
}

fn create_grub_iso(output_path : String, input_directory : String) -> std::io::Result<()>
{
    let tree = construct_directory(PathBuf::from(input_directory)).unwrap();
    println!("{:?}", tree);

    let volume_descriptor_list = generate_volume_descriptors();

    let mut out_file = File::create(output_path)?;

    // First we have the System Area, that is unused
    let buffer : [u8; 0x8000] = [0; 0x8000];

    out_file.write_all(&buffer)?;
    for mut volume in volume_descriptor_list
    {
        volume.write_volume(&mut out_file)?;
    }

    // TODO everything

    Ok(())
}