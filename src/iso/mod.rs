#[macro_use]
mod utils;
mod directory_entry;
mod file_entry;
pub mod option;
mod volume_descriptor;

use byteorder::{BigEndian, LittleEndian, WriteBytesExt};

use std;

use directory_entry::DirectoryEntry;
use file_entry::{FileEntry, FileType};
use std::fs::File;
use std::io::prelude::*;
use std::io::Cursor;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::str::FromStr;
use utils::SECTOR_SIZE;
use utils::{LOGIC_SIZE, LOGIC_SIZE_U32};
use volume_descriptor::VolumeDescriptor;

fn assign_directory_identifiers(
    tree: &mut DirectoryEntry,
    last_index: &mut u32,
    last_lba: &mut u32,
) {
    if *last_index == 0 {
        tree.parent_index = *last_index;
        tree.path_table_index = *last_index + 1;

        *last_index = tree.path_table_index;
    } else {
        tree.lba = *last_lba;
    }
    *last_lba += tree.get_extent_size_in_lb();

    for entry in &mut tree.dir_childs {
        entry.parent_index = tree.path_table_index;
        entry.path_table_index = *last_index + 1;

        *last_index = entry.path_table_index;
    }

    for entry in &mut tree.dir_childs {
        assign_directory_identifiers(entry, last_index, last_lba);
    }
}

fn reserve_file_space(directory_entry: &mut DirectoryEntry, current_lba: &mut u32) {
    for child_directory in &mut directory_entry.dir_childs {
        reserve_file_space(child_directory, current_lba);
    }

    for child_file in &mut directory_entry.files_childs {
        let lba_count = ((child_file.size as u32) + LOGIC_SIZE_U32) / LOGIC_SIZE_U32;
        child_file.lba = *current_lba;
        *current_lba += lba_count;
    }
}

fn generate_volume_descriptors(opt: &option::Opt) -> Vec<VolumeDescriptor> {
    let mut res: Vec<VolumeDescriptor> = Vec::new();

    res.push(VolumeDescriptor::Primary);
    if opt.eltorito_opt.eltorito_boot.is_some() {
        res.push(VolumeDescriptor::Boot);
    }
    res.push(VolumeDescriptor::End);

    res
}

fn create_boot_catalog(tree: &mut DirectoryEntry) {
    let catalog_file = FileEntry::new_buffered(String::from("boot.cat"));
    tree.add_file(catalog_file);
}

fn fill_boot_catalog(tree: &mut DirectoryEntry, opt: &mut option::Opt) -> std::io::Result<()> {
    let value = opt.eltorito_opt.eltorito_boot.clone().unwrap();
    let eltorito_boot_file: &mut FileEntry = tree.get_file(&value).unwrap();
    let mut sector_count = ((eltorito_boot_file.size as u32) + SECTOR_SIZE) / SECTOR_SIZE;

    // align to LB if not enough
    if sector_count < 4 {
        sector_count = 4;
    }

    let eltorito_lba = eltorito_boot_file.lba;

    let file: &mut FileEntry = tree.get_file("boot.cat").unwrap();

    let mut buff: Vec<u8> = Vec::new();

    // Validation Header

    // Header ID
    buff.write_u8(0x1)?;

    // Plateform ID (0x0 = 80x86, 0x1 = PowerPC, 0x2 = Mac, 0xef = EFI)
    // TODO: UEFI?
    buff.write_u8(0x0)?;

    // Reserved
    buff.write_u16::<LittleEndian>(0x0)?;

    let id_str: [u8; 0x16] = [0x0; 0x16];
    buff.write_all(&id_str)?;

    // FIXME: actually calculate the checksum correctly!
    buff.write_u32::<LittleEndian>(0x55aa_0000)?;

    buff.write_u8(0x55)?;
    buff.write_u8(0xAA)?;

    let boot_indicator = if opt.eltorito_opt.no_boot { 0x0 } else { 0x88 };

    buff.write_u8(boot_indicator)?;

    // Boot medium type (force no emu mode)
    buff.write_u8(0x0)?;

    // Load segment (0 means default, 0x7C0. As we don't manage any emulation mode, we don't care of it)
    buff.write_u16::<LittleEndian>(0x0)?;

    // System Type. "This must be a copy of byte 5 (System Type) from the Partition Table found in the boot image."
    // As we don't emulate harddrive, this is 0 here
    buff.write_u8(0x0)?;

    // Unused (0xC - 0x1F)
    buff.write_u8(0x0)?;

    // Sector count
    buff.write_u16::<LittleEndian>(sector_count as u16)?;

    // LBA of the file
    buff.write_u32::<LittleEndian>(eltorito_lba)?;

    let unused: [u8; 0x14] = [0x0; 0x14];
    // Unused
    buff.write_all(&unused)?;

    file.file_type = match &file.file_type {
        FileType::Buffer { name, .. } => FileType::Buffer {
            name: name.clone(),
            data: buff,
        },
        _ => panic!(),
    };
    file.update();

    Ok(())
}

fn patch_boot_image(tree: &mut DirectoryEntry, opt: &mut option::Opt) -> std::io::Result<()> {
    let value = opt.eltorito_opt.eltorito_boot.clone().unwrap();
    let file: &mut FileEntry = tree.get_file(&value).unwrap();

    // We need to copy the file to a buffer and change the file type internally to be able to patch it
    let mut content: Box<Read> = file.open_content_provider();
    let mut buff: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    std::io::copy(&mut content, &mut buff)?;

    // Path the content now
    buff.seek(SeekFrom::Start(0x8))?;

    // LBA of primary volume descriptor (always 0x10 in our case)
    buff.write_u32::<LittleEndian>(0x10)?;

    // LBA of boot file.
    buff.write_u32::<LittleEndian>(file.lba)?;

    // Length of boot file.
    buff.write_u32::<LittleEndian>(file.size as u32)?;

    // Checksum (actually ignored by GRUB2)
    // FIXME: should we implement it?
    buff.write_u32::<LittleEndian>(0x0)?;

    file.file_type = FileType::Buffer {
        name: file.get_file_name(),
        data: buff.into_inner(),
    };
    file.update();

    Ok(())
}

fn write_system_area<T>(output_writter: &mut T, opt: &option::Opt) -> std::io::Result<()>
where
    T: Write + Seek,
{
    let old_pos = output_writter.seek(SeekFrom::Current(0))? as usize;

    if opt.embedded_boot.is_some() {
        let embedded_boot = opt.embedded_boot.clone().unwrap();
        let path: PathBuf = PathBuf::from_str(&embedded_boot).unwrap();
        if path.metadata().unwrap().len() > (LOGIC_SIZE * 0x10) as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "generic boot file is bigger than 32768 bytes!",
            ));
        }
        let mut embedded_boot_file = File::open(path)?;
        std::io::copy(&mut embedded_boot_file, output_writter)?;
    }

    // Pad to 0x8000 if needed
    let current_pos = output_writter.seek(SeekFrom::Current(0))? as usize;
    let diff_size = current_pos - old_pos;

    if diff_size != LOGIC_SIZE * 0x10 {
        let mut padding: Vec<u8> = Vec::new();
        padding.resize(LOGIC_SIZE * 0x10 - diff_size, 0u8);
        output_writter.write_all(&padding)?;
    }

    Ok(())
}

pub fn create_iso(opt: &mut option::Opt) -> std::io::Result<()> {
    let volume_descriptor_list = generate_volume_descriptors(opt);

    let mut out_file = File::create(&opt.output)?;

    let mut current_lba: u32 = 0x10 + 1 + (volume_descriptor_list.len() as u32);

    let path_table_start_lba = current_lba;

    // Reserve 4 LBA for path tables (add some spacing after table)
    current_lba += 4;

    let mut tree = DirectoryEntry::new(PathBuf::from(&opt.input_directory))?;
    let mut path_table_index = 0;

    let mut tmp_lba = current_lba;

    assign_directory_identifiers(&mut tree, &mut path_table_index, &mut tmp_lba);
    tree.parent_index = 1;
    tree.lba = current_lba;
    tree.print();

    current_lba = tmp_lba;
    current_lba += 1;

    if opt.eltorito_opt.eltorito_boot.is_some() {
        create_boot_catalog(&mut tree);
    }

    reserve_file_space(&mut tree, &mut current_lba);

    if opt.eltorito_opt.eltorito_boot.is_some() {
        fill_boot_catalog(&mut tree, opt)?;
    }

    if opt.eltorito_opt.boot_info_table {
        patch_boot_image(&mut tree, opt)?;
    }

    write_system_area(&mut out_file, opt)?;

    for mut volume in volume_descriptor_list {
        volume.write_volume(&mut out_file, &mut tree, path_table_start_lba, current_lba)?;
    }

    // FIXME: what is this and why do I need it???? checksum infos??
    let empty_mki_section: [u8; 2044] = [0; 2044];
    out_file.write_all(b"MKI ")?;
    out_file.write_all(&empty_mki_section)?;

    tree.write_path_table::<File, LittleEndian>(&mut out_file, path_table_start_lba)?;
    tree.write_path_table::<File, BigEndian>(&mut out_file, path_table_start_lba + 1)?;
    tree.write_extent(&mut out_file, None)?;
    tree.write_files(&mut out_file)?;

    Ok(())
}
