//! Write exact browser integers without allocating a second JSON value tree.
use serde::Serialize;
use serde_json::ser::{CompactFormatter, Formatter};
use std::io::{self, Write};

#[derive(Default)]
struct DecimalIntegers {
    in_string: bool,
}
macro_rules! integer {
    ($method:ident, $ty:ty) => {
        fn $method<W: ?Sized + Write>(&mut self, writer: &mut W, value: $ty) -> io::Result<()> {
            if !self.in_string {
                writer.write_all(b"\"")?;
            }
            CompactFormatter.$method(writer, value)?;
            if !self.in_string {
                writer.write_all(b"\"")?;
            }
            Ok(())
        }
    };
}
impl Formatter for DecimalIntegers {
    fn begin_string<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.in_string = true;
        CompactFormatter.begin_string(writer)
    }
    fn end_string<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.in_string = false;
        CompactFormatter.end_string(writer)
    }
    integer!(write_i8, i8);
    integer!(write_i16, i16);
    integer!(write_i32, i32);
    integer!(write_i64, i64);
    integer!(write_i128, i128);
    integer!(write_u8, u8);
    integer!(write_u16, u16);
    integer!(write_u32, u32);
    integer!(write_u64, u64);
    integer!(write_u128, u128);
}
pub(super) fn write(writer: impl Write, value: &impl Serialize) -> crate::Result<()> {
    let mut serializer = serde_json::Serializer::with_formatter(writer, DecimalIntegers::default());
    value.serialize(&mut serializer)?;
    Ok(())
}
