#[macro_use]
mod utils;
mod directory_entry;
mod file_entry;
mod volume_descriptor;
pub mod option;

use byteorder::{BigEndian, LittleEndian};

use std;

use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

use directory_entry::DirectoryEntry;
use utils::LOGIC_SIZE_U32;
use volume_descriptor::VolumeDescriptor;

fn assign_directory_identifiers(tree: &mut DirectoryEntry, last_index: &mut u32, current_lba: u32) {
    if *last_index == 0 {
        tree.parent_index = *last_index;
        tree.path_table_index = *last_index + 1;

        *last_index = tree.path_table_index;
    } else {
        tree.lba = current_lba + tree.path_table_index;
    }

    for entry in &mut tree.dir_childs {
        entry.parent_index = tree.path_table_index;
        entry.path_table_index = *last_index + 1;

        *last_index = entry.path_table_index;
    }

    for entry in &mut tree.dir_childs {
        assign_directory_identifiers(entry, last_index, current_lba);
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

fn generate_volume_descriptors() -> Vec<VolumeDescriptor> {
    let mut res: Vec<VolumeDescriptor> = Vec::new();

    res.push(VolumeDescriptor::Primary);
    res.push(VolumeDescriptor::End);

    res
}

pub fn create_iso(opt: &mut option::Opt) -> std::io::Result<()> {
    let volume_descriptor_list = generate_volume_descriptors();

    let mut out_file = File::create(&opt.output)?;

    // First we have the System Area, that is unused
    let buffer: [u8; utils::LOGIC_SIZE * 0x10] = [0; utils::LOGIC_SIZE * 0x10];

    let mut current_lba: u32 = 0x10 + 1 + (volume_descriptor_list.len() as u32);

    let path_table_start_lba = current_lba;

    // Reserve 4 LBA for path tables (add some spacing after table)
    current_lba += 4;

    let mut tree = DirectoryEntry::new(PathBuf::from(&opt.input_directory))?;
    let mut path_table_index = 0;

    assign_directory_identifiers(&mut tree, &mut path_table_index, current_lba - 1);
    tree.parent_index = 1;
    tree.lba = current_lba;

    current_lba += path_table_index;
    current_lba += 1;

    reserve_file_space(&mut tree, &mut current_lba);

    out_file.write_all(&buffer)?;
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
