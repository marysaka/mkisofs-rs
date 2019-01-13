pub const LOGIC_SIZE: usize = 0x800;
pub const LOGIC_SIZE_I64: i64 = 0x800;
pub const LOGIC_SIZE_U32: u32 = 0x800;
pub const LOGIC_SIZE_U16: u16 = 0x800;

pub fn align_up(value: i32, padding: i32) -> i32 {
    (value + (padding - 1)) & -padding
}

macro_rules! write_bothendian {
    ($($writer:ident . $write_fn:ident($value:expr)?;)*) => {
        $($writer.$write_fn::<LittleEndian>($value)?;)*
        $($writer.$write_fn::<BigEndian>($value)?;)*
    }
}
