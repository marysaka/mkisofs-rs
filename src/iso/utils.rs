pub const LOGIC_SIZE: usize = 0x800;
pub const LOGIC_SIZE_I64: i64 = 0x800;
pub const LOGIC_SIZE_U32: u32 = 0x800;
pub const LOGIC_SIZE_U16: u16 = 0x800;

pub fn align_up(value: i32, padding: i32) -> i32 {
    (value + (padding - 1)) & -padding
}

pub fn convert_name(value: &str) -> Vec<u8> {
    let res: Vec<&str> = value.split('.').collect();

    let file_name: &str = &res[0];
    let file_truncated_size = if file_name.len() >= 8 {
        8
    } else {
        file_name.len()
    };

    let extension: &str = res.get(1).unwrap_or(&"");
    let extension_truncated_size = if extension.len() > 3 {
        3
    } else {
        extension.len()
    };

    let mut result = String::from(&file_name[0..file_truncated_size]);
    if extension_truncated_size != 0 {
        result = result + "." + &extension[0..extension_truncated_size];
    }

    result.into_bytes()
}

pub fn get_entry_size(base_size: u32, file_name: &str, directory_type: u32, padding_type: usize) -> u32
{
    let file_name_corrected = convert_name(file_name);
    let file_identifier = match directory_type {
        1 => &[0u8],
        2 => &[1u8],
        _ => {
            &file_name_corrected[..]
        }
    };
    let mut file_identifier_len = file_identifier.len();

    if file_identifier_len % 2 != padding_type {
        file_identifier_len += 1;
    }


    base_size + file_identifier_len as u32
}

macro_rules! write_bothendian {
    ($($writer:ident . $write_fn:ident($value:expr)?;)*) => {
        $($writer.$write_fn::<LittleEndian>($value)?;)*
        $($writer.$write_fn::<BigEndian>($value)?;)*
    }
}
