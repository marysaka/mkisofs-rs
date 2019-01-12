extern crate byteorder;
extern crate chrono;

use byteorder::{LittleEndian, BigEndian, ByteOrder, WriteBytesExt};
use chrono::prelude::*;

use std;
use std::env;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::fs::File;
use std::fs::DirEntry;
use std::fs::Metadata;
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct FileEntry
{
    pub path: PathBuf,
    pub size: usize,
    pub lba: u32,
    pub aligned_size: usize
}

#[derive(Debug, Clone)]
struct DirectoryEntry
{
    pub path_table_index : u32,
    pub parent_index : u32,
    pub path: PathBuf,
    pub dir_childs : Vec<DirectoryEntry>,
    pub files_childs : Vec<FileEntry>,
    pub lba : u32
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

pub fn align_up(value: i32, padding: i32) -> i32 {
    (value + (padding - 1)) & -padding
}

fn construct_directory(path : PathBuf) -> std::io::Result<DirectoryEntry>
{
    let dir_path = path.clone();
    let mut dir_childs : Vec<DirectoryEntry> = Vec::new();
    let mut files_childs : Vec<FileEntry> = Vec::new();

    let mut ordered_dir: Vec<DirEntry> = fs::read_dir(path)?
                                              .map(|r| r.unwrap())
                                              .collect();
    ordered_dir.sort_by_key(|dir| dir.path());

    for entry in ordered_dir {
        let entry_meta : Metadata = entry.metadata()?;
        if entry_meta.is_dir() {
            dir_childs.push(construct_directory(entry.path())?);
        } else if entry_meta.is_file() {
            files_childs.push(FileEntry { path: entry.path(), size: entry_meta.len() as usize, lba: 0, aligned_size: align_up(entry_meta.len() as i32, 0x800) as usize})
        }
    }
    Ok(DirectoryEntry {path_table_index: 0, parent_index: 0, path: dir_path, dir_childs, files_childs, lba: 0})
}

fn assign_directory_identifiers(tree: &mut DirectoryEntry, last_index: &mut u32, current_lba: u32)
{
    if *last_index == 0
    {
        tree.parent_index = *last_index;
        tree.path_table_index = *last_index + 1;

        *last_index = tree.path_table_index;
    }
    else
    {
        tree.lba = current_lba + tree.path_table_index;
    }

    for entry in &mut tree.dir_childs
    {
        entry.parent_index = tree.path_table_index;
        entry.path_table_index = *last_index + 1;

        *last_index = entry.path_table_index;
    }

    for entry in &mut tree.dir_childs
    {
        assign_directory_identifiers(entry, last_index, current_lba);
    }
}

fn reserve_file_space(directory_entry : &mut DirectoryEntry, current_lba : &mut u32)
{
    for child_directory in &mut directory_entry.dir_childs
    {
        reserve_file_space(child_directory, current_lba);
    }

    for child_file in &mut directory_entry.files_childs
    {
        let lba_count = (child_file.size + 0x800) / 0x800;
        child_file.lba = *current_lba;
        *current_lba += lba_count as u32;
    }
}

#[derive(Debug)]
enum VolumeDescriptor
{
    Boot,
    Primary,
    Supplementary,
    Volume,
    End
}

macro_rules! write_multiendian {
    ($($writer:ident . $write_fn:ident($value:expr)?;)*) => {
        $($writer.$write_fn::<LittleEndian>($value)?;)*
        $($writer.$write_fn::<BigEndian>($value)?;)*
    }
}

impl FileEntry
{
    fn write_entry<T>(&self, output_writter: &mut T) -> std::io::Result<()> where T: Write
    {
        // TODO: CONVERT IT TO VALID DATA
        let file_name = self.path.file_name().unwrap().to_str().unwrap().to_uppercase();
        let file_identifier = file_name.as_bytes();
        let file_identifier_len = file_identifier.len() + 2;

        let file_identifier_padding = match (file_identifier_len % 2) == 0 {
            true => 1,
            false => 0
        };

        let entry_size : u8 = 0x21u8 + (file_identifier_len as u8) + file_identifier_padding;

        output_writter.write_u8(entry_size)?;

        // Extended Attribute Record length. 
        output_writter.write_u8(0u8)?;

        // Location of extent (LBA) in both-endian format. 
        output_writter.write_u32::<LittleEndian>(self.lba)?;
        output_writter.write_u32::<BigEndian>(self.lba)?;

        output_writter.write_u32::<LittleEndian>(self.size as u32)?;
        output_writter.write_u32::<BigEndian>(self.size as u32)?;

        let record_datetime: DateTime<Utc> = Utc::now();
        output_writter.write_u8((record_datetime.year() - 1900) as u8)?;
        output_writter.write_u8((record_datetime.month()) as u8)?;
        output_writter.write_u8((record_datetime.day()) as u8)?;
        output_writter.write_u8((record_datetime.hour()) as u8)?;
        output_writter.write_u8((record_datetime.minute()) as u8)?;
        output_writter.write_u8((record_datetime.second()) as u8)?;
        output_writter.write_u8(0u8)?;

        // file flags
        output_writter.write_u8(0x0u8)?;

        output_writter.write_u8(0x0u8)?;
        output_writter.write_u8(0x0u8)?;

        output_writter.write_u16::<LittleEndian>(0x1)?;
        output_writter.write_u16::<BigEndian>(0x1)?;

        output_writter.write_u8(file_identifier_len as u8)?;
        output_writter.write_all(file_identifier)?;
        output_writter.write_all(b";1")?;

        // padding if even
        if (file_identifier_len % 2) == 0 {
            output_writter.write_u8(0x0u8)?;
        }

        Ok(())
    }

    fn write_content<T>(&mut self, output_writter: &mut T) -> std::io::Result<()> where T: Write + Seek
    {
        let old_pos = output_writter.seek(SeekFrom::Current(0))?;

        // Seek to the correct LBA
        output_writter.seek(SeekFrom::Start((self.lba * 0x800) as u64))?;

        // TODO support other content provider
        let mut file = File::open(&self.path)?;

        io::copy(&mut file, output_writter)?;

        let current_pos = output_writter.seek(SeekFrom::Current(0))? as usize;
        let expected_aligned_pos = ((current_pos as i64) & -0x800) as usize;

        let diff_size = current_pos - expected_aligned_pos;

        if diff_size != 0
        {
            let mut padding : Vec<u8> = Vec::new();
            padding.resize(0x800 - diff_size, 0u8);
            output_writter.write(&padding)?;
        }

        output_writter.seek(SeekFrom::Start(old_pos))?;

        Ok(())
    }
}

impl DirectoryEntry
{
    fn write_entry<T>(directory_entry : &DirectoryEntry, output_writter: &mut T, directory_type : u32) -> std::io::Result<()> where T: Write
    {
        let file_name = directory_entry.path.file_name().unwrap().to_str().unwrap().to_uppercase();

        let file_identifier = match directory_type {
            1 => &[0u8],
            2 => &[1u8],
            _ => {
                // TODO: CONVERT IT TO VALID DATA
                file_name.as_bytes()
            }
        };

        let file_identifier_len = file_identifier.len();
        let file_identifier_padding = match (file_identifier_len % 2) == 0 {
            true => 1,
            false => 0
        };

        let entry_size : u8 = match directory_type {
            0 => {
                0x21u8 + (file_identifier_len as u8) + file_identifier_padding
            },
            _ => 0x22u8
        };

        output_writter.write_u8(entry_size)?;

        // Extended Attribute Record length. 
        output_writter.write_u8(0u8)?;

        // Location of extent (LBA) in both-endian format. 
        output_writter.write_u32::<LittleEndian>(directory_entry.lba)?;
        output_writter.write_u32::<BigEndian>(directory_entry.lba)?;

        output_writter.write_u32::<LittleEndian>(0x800)?;
        output_writter.write_u32::<BigEndian>(0x800)?;

        let record_datetime: DateTime<Utc> = Utc::now();
        output_writter.write_u8((record_datetime.year() - 1900) as u8)?;
        output_writter.write_u8((record_datetime.month()) as u8)?;
        output_writter.write_u8((record_datetime.day()) as u8)?;
        output_writter.write_u8((record_datetime.hour()) as u8)?;
        output_writter.write_u8((record_datetime.minute()) as u8)?;
        output_writter.write_u8((record_datetime.second()) as u8)?;
        output_writter.write_u8(0u8)?;

        // file flags (0x2 == directory)
        output_writter.write_u8(0x2u8)?;

        output_writter.write_u8(0x0u8)?;
        output_writter.write_u8(0x0u8)?;

        output_writter.write_u16::<LittleEndian>(0x1)?;
        output_writter.write_u16::<BigEndian>(0x1)?;

        output_writter.write_u8(file_identifier_len as u8)?;
        output_writter.write_all(file_identifier)?;

        // padding if even
        if (file_identifier_len % 2) == 0 {
            output_writter.write_u8(0x0u8)?;
        }

        Ok(())
    }

    fn len(&self) -> u32
    {
        let mut res = 0;

        for entry in &self.dir_childs
        {
            res += entry.len();
        }

        res
    }

    fn get_path_table_size(&self) -> u32
    {
        let mut res = 0x8u32;

        let file_name = self.path.file_name().unwrap().to_str().unwrap().to_uppercase();
        let file_identifier = match self.path_table_index {
            1 => &[0u8],
            _ => {
                // TODO: CONVERT IT TO VALID DATA
                file_name.as_bytes()
            }
        };
        let mut file_identifier_len = file_identifier.len();
        if (file_identifier_len % 2) != 0 {
            file_identifier_len += 1;
        }

        res += file_identifier_len as u32;

        for entry in &self.dir_childs
        {
            res += entry.get_path_table_size();
        }

        res
    }

    fn write_path_table_entry<T, Order: ByteOrder>(directory_entry : &DirectoryEntry, output_writter: &mut T, directory_type : u32) -> std::io::Result<()> where T: Write
    {
        let file_name = directory_entry.path.file_name().unwrap().to_str().unwrap().to_uppercase();

        let file_identifier = match directory_type {
            1 => &[0u8],
            _ => {
                // TODO: CONVERT IT TO VALID DATA
                file_name.as_bytes()
            }
        };

        let file_identifier_len = file_identifier.len();

        output_writter.write_u8(file_identifier_len as u8)?;
        output_writter.write_u8(0x0u8)?;
        output_writter.write_u32::<Order>(directory_entry.lba)?;
        output_writter.write_u16::<Order>(directory_entry.parent_index as u16)?;
        output_writter.write_all(&file_identifier)?;

        // padding if odd
        if (file_identifier_len % 2) != 0 {
            output_writter.write_u8(0x0u8)?;
        }

        Ok(())
    }

    fn write_path_table_childs<T, Order: ByteOrder>(&mut self, output_writter: &mut T) -> std::io::Result<()> where T: Write
    {
        for entry in &mut self.dir_childs
        {
            DirectoryEntry::write_path_table_entry::<T, Order>(entry, output_writter, 0)?;
        }

        for entry in &mut self.dir_childs
        {
            entry.write_path_table_childs::<T, Order>(output_writter)?;
        }

        Ok(())
    }

    fn write_path_table<T, Order: ByteOrder>(&mut self, output_writter: &mut T, path_table_pos: u32) -> std::io::Result<()> where T: Write + Seek
    {
        let old_pos = output_writter.seek(SeekFrom::Current(0))?;

        // Seek to the correct LBA
        output_writter.seek(SeekFrom::Start((path_table_pos * 0x800) as u64))?;

        let old_pos_current_context = output_writter.seek(SeekFrom::Current(0))?;


        // Write root
        DirectoryEntry::write_path_table_entry::<T, Order>(self, output_writter, 1)?;

        self.write_path_table_childs::<T, Order>(output_writter)?;

        // Pad to LBA size
        let current_pos = output_writter.seek(SeekFrom::Current(0))? as usize;
        let expected_aligned_pos = ((current_pos as i64) & -0x800) as usize;

        let diff_size = current_pos - expected_aligned_pos;

        let written_size = current_pos - (old_pos_current_context as usize);
        assert!(written_size == (self.get_path_table_size() as usize));

        if diff_size != 0
        {
            let mut padding : Vec<u8> = Vec::new();
            padding.resize(0x800 - diff_size, 0u8);
            output_writter.write(&padding)?;
        }

        // Restore old position
        output_writter.seek(SeekFrom::Start(old_pos))?;

        Ok(())
    }

    fn write_extent<T>(&mut self, output_writter: &mut T, parent_option : Option<&DirectoryEntry>) -> std::io::Result<()> where T: Write + Seek
    {
        let old_pos = output_writter.seek(SeekFrom::Current(0))?;

        // Seek to the correct LBA
        output_writter.seek(SeekFrom::Start((self.lba * 0x800) as u64))?;

        self.write_as_current(output_writter)?;

        let mut empty_parent_path = PathBuf::new();
        empty_parent_path.set_file_name("dummy"); 
        let empty_parent = DirectoryEntry {path_table_index: 0, parent_index: 0, path: empty_parent_path, dir_childs: Vec::new(), files_childs: Vec::new(), lba: self.lba};

        let parent = match parent_option
        {
            Some(res) => res,
            None => &empty_parent
        };

        parent.write_as_parent(output_writter)?;

        // FIXME: dirty
        let self_clone = self.clone();

        for child_directory in &mut self.dir_childs
        {
            child_directory.write_one(output_writter)?;
            child_directory.write_extent(output_writter, Some(&self_clone))?;
        }

        for child_file in &mut self.files_childs
        {
            child_file.write_entry(output_writter)?;
        }

        // Pad to LBA size
        let current_pos = output_writter.seek(SeekFrom::Current(0))? as usize;
        let expected_aligned_pos = ((current_pos as i64) & -0x800) as usize;

        let diff_size = current_pos - expected_aligned_pos;

        if diff_size != 0
        {
            let mut padding : Vec<u8> = Vec::new();
            padding.resize(0x800 - diff_size, 0u8);
            output_writter.write(&padding)?;
        }

        // Restore old position
        output_writter.seek(SeekFrom::Start(old_pos))?;

        Ok(())
    }

    fn write_files<T>(&mut self, output_writter: &mut T) -> std::io::Result<()> where T: Write + Seek
    {
        for child_directory in &mut self.dir_childs
        {
            child_directory.write_files(output_writter)?;
        }

        for child_file in &mut self.files_childs
        {
            child_file.write_content(output_writter)?;
        }
        Ok(())
    }

    fn write_as_current<T>(&self, output_writter: &mut T) -> std::io::Result<()> where T: Write
    {
        DirectoryEntry::write_entry(self, output_writter, 1)
    }

    fn write_as_parent<T>(&self, output_writter: &mut T) -> std::io::Result<()> where T: Write
    {
        DirectoryEntry::write_entry(self, output_writter, 2)
    }

    fn write_one<T>(&self, output_writter: &mut T) -> std::io::Result<()> where T: Write
    {
        DirectoryEntry::write_entry(self, output_writter, 0)
    }
}

impl VolumeDescriptor
{
    fn get_type_id(&self) -> u8
    {
        match self
        {
            VolumeDescriptor::Boot => 0,
            VolumeDescriptor::Primary => 1,
            VolumeDescriptor::Supplementary => 2,
            VolumeDescriptor::Volume => 3,
            VolumeDescriptor::End => 0xff
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

    fn write_volume<T>(&mut self, output_writter: &mut T, root_dir : &mut DirectoryEntry, path_table_start_lba: u32, size_in_lb: u32) -> std::io::Result<()> where T: Write
    {
        self.write_volume_header(output_writter)?;

        match self
        {
            VolumeDescriptor::Primary => {
                output_writter.write_u8(0)?;

                let system_identifier : [u8; 32] = [0x20; 32];
                output_writter.write_all(&system_identifier)?;

                output_writter.write_all(b"ISOIMAGE                        ")?;
                output_writter.write_u64::<LittleEndian>(0)?;
                
                output_writter.write_u32::<LittleEndian>(size_in_lb)?;
                output_writter.write_u32::<BigEndian>(size_in_lb)?;

                let zero_b32 : [u8; 32] = [0; 32];
                output_writter.write_all(&zero_b32)?;

                // Disc count
                output_writter.write_u16::<LittleEndian>(1)?;
                output_writter.write_u16::<BigEndian>(1)?;

                // Disc id
                output_writter.write_u16::<LittleEndian>(1)?;
                output_writter.write_u16::<BigEndian>(1)?;

                // logic size: 2KB
                output_writter.write_u16::<LittleEndian>(0x800)?;
                output_writter.write_u16::<BigEndian>(0x800)?;

                let path_table_size = root_dir.get_path_table_size();
                // TODO: path table size
                output_writter.write_u32::<LittleEndian>(path_table_size)?;
                output_writter.write_u32::<BigEndian>(path_table_size)?;

                // path table location (in lba)
                let path_table_lba_le = path_table_start_lba;     // System Area + Primary + End
                let path_table_lba_be = path_table_start_lba + 2; // System Area + Primary + End + Path Table LE + Spacing

                output_writter.write_u32::<LittleEndian>(path_table_lba_le)?;
                output_writter.write_u32::<LittleEndian>(0)?;
                output_writter.write_u32::<BigEndian>(path_table_lba_be)?;
                output_writter.write_u32::<BigEndian>(0)?;

                root_dir.write_as_current(output_writter)?;

                let volume_set_identifier : [u8; 128] = [0x20; 128];
                let publisher_identifier : [u8; 128] = [0x20; 128];
                let data_preparer_identifier : [u8; 128] = [0x20; 128];
                let application_identifier : [u8; 128] = [0x20; 128];
                output_writter.write_all(&volume_set_identifier)?;
                output_writter.write_all(&publisher_identifier)?;
                output_writter.write_all(&data_preparer_identifier)?;
                output_writter.write_all(&application_identifier)?;

                let copyright_file_identifier : [u8; 38] = [0x20; 38];
                let abstract_file_identifier  : [u8; 36] = [0x20; 36];
                let bibliographic_file_identifier : [u8; 37] = [0x20; 37];
                output_writter.write_all(&copyright_file_identifier)?;
                output_writter.write_all(&abstract_file_identifier)?;
                output_writter.write_all(&bibliographic_file_identifier)?;

                let utc: DateTime<Utc> = Utc::now();
                let creation_time : String = utc.format("%Y%m%d%H%M%S00").to_string();
                let expiration_time : [u8; 16] = [0x30; 16];

                output_writter.write_all(creation_time.as_bytes())?;
                output_writter.write_u8(0)?;
                output_writter.write_all(creation_time.as_bytes())?;
                output_writter.write_u8(0)?;
                output_writter.write_all(&expiration_time)?;
                output_writter.write_u8(0)?;
                output_writter.write_all(&expiration_time)?;
                output_writter.write_u8(0)?;

                // File structure version
                output_writter.write_u8(0x1)?;

                output_writter.write_u8(0x0)?;

                let application_used : [u8; 512] = [0x20; 512];
                output_writter.write_all(&application_used)?;
                
                let reserved : [u8; 653] = [0x0; 653];
                output_writter.write_all(&reserved)?;
            },
            VolumeDescriptor::End => {
                let empty_data : [u8; 2041] = [0; 2041];
                output_writter.write_all(&empty_data)?;
            },
            _ => unimplemented!()
        }
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

fn print_tree(tree : &DirectoryEntry)
{
    for entry in &tree.dir_childs
    {
        println!("{:?}: {} {} ({:x})", entry.path, entry.parent_index, entry.path_table_index, entry.lba);
    }

    for entry in &tree.dir_childs
    {
        print_tree(entry);
    }
}

fn create_grub_iso(output_path : String, input_directory : String) -> std::io::Result<()>
{

    let volume_descriptor_list = generate_volume_descriptors();

    let mut out_file = File::create(output_path)?;

    // First we have the System Area, that is unused
    let buffer : [u8; 0x8000] = [0; 0x8000];

    // TODO: Path Table
    let mut current_lba : u32 = 0x10 + 1 + (volume_descriptor_list.len() as u32);

    let path_table_start_lba = current_lba;
    
    // Reserve 4 LBA for path tables (add some spacing after table)
    current_lba += 4;

    let mut tree = construct_directory(PathBuf::from(input_directory))?;
    let mut path_table_index = 0;

    assign_directory_identifiers(&mut tree, &mut path_table_index, current_lba - 1);
    tree.parent_index = 1;
    tree.lba = current_lba;
    println!("{:?}: {} {} ({:x})", tree.path, tree.parent_index, tree.path_table_index, tree.lba);
    print_tree(&tree);

    current_lba += path_table_index;
    current_lba += 1;

    reserve_file_space(&mut tree, &mut current_lba);

    out_file.write_all(&buffer)?;
    for mut volume in volume_descriptor_list
    {
        volume.write_volume(&mut out_file, &mut tree, path_table_start_lba, current_lba)?;
    }

    // FIXME: what is this and why do I need it???? checksum infos??
    let empty_mki_section : [u8; 2044] = [0; 2044];
    out_file.write_all(b"MKI ")?;
    out_file.write_all(&empty_mki_section)?;

    // TODO: Path table LE/BE
    tree.write_path_table::<File, LittleEndian>(&mut out_file, path_table_start_lba)?;
    tree.write_path_table::<File, BigEndian>(&mut out_file, path_table_start_lba + 1)?;
    tree.write_extent(&mut out_file, None)?;
    tree.write_files(&mut out_file)?;

    Ok(())
}