use crate::core::PowerStateSubsystemSleepState;

pub const DESCRIPTOR: &str = "vendor.oplus.hardware.rpmh.IRpmh";
pub const INTERFACE_VERSION: i32 = 1;
pub const INTERFACE_HASH: &str = "acf5f9a3960fb9a2f76f86947dff5ca9e0c86e05";
pub const STATUS_BAD_VALUE: i32 = -22;
pub const STATUS_UNKNOWN_TRANSACTION: i32 = -74;
pub const STATUS_UNEXPECTED_NULL: i32 = i32::MIN + 8;
pub const MAX_STRING_BYTES: usize = 4096;
pub const MAX_LIST_STATES: usize = 7;

pub fn consumed(position: i32, size: i32) -> Result<(), i32> {
    if position < 0 || !(0..=1 << 20).contains(&size) || position != size {
        Err(STATUS_BAD_VALUE)
    } else {
        Ok(())
    }
}
pub fn string_length(length: i32) -> Result<Option<usize>, i32> {
    if length == -1 {
        return Ok(None);
    }
    if length <= 0 || length as usize > MAX_STRING_BYTES + 1 {
        return Err(STATUS_BAD_VALUE);
    }
    Ok(Some(length as usize))
}
pub fn parcelable_size(start: i32, end: i32) -> Result<i32, i32> {
    let size = end.checked_sub(start).ok_or(STATUS_BAD_VALUE)?;
    if start < 0 || !(4..=1 << 20).contains(&size) {
        Err(STATUS_BAD_VALUE)
    } else {
        Ok(size)
    }
}

pub trait ReplyWriter {
    type Error;
    fn invalid_value(&self) -> Self::Error;
    fn ok_status(&mut self) -> Result<(), Self::Error>;
    fn int32(&mut self, value: i32) -> Result<(), Self::Error>;
    fn int64(&mut self, value: i64) -> Result<(), Self::Error>;
    fn string(&mut self, value: &str) -> Result<(), Self::Error>;
    fn begin_state(&mut self) -> Result<i32, Self::Error>;
    fn end_state(&mut self, start: i32) -> Result<(), Self::Error>;
}

fn state<W: ReplyWriter>(
    out: &mut W,
    value: &PowerStateSubsystemSleepState,
) -> Result<(), W::Error> {
    let start = out.begin_state()?;
    out.string(&value.name)?;
    out.int64(value.version)?;
    out.int64(value.residency_in_msec_since_boot)?;
    out.int64(value.total_transitions)?;
    out.int64(value.last_entry_timestamp_ms)?;
    out.int32(i32::from(value.supported_only_in_suspend))?;
    out.end_state(start)
}

pub fn single<W: ReplyWriter>(
    out: &mut W,
    result: i32,
    value: &PowerStateSubsystemSleepState,
) -> Result<(), W::Error> {
    out.ok_status()?;
    out.int32(result)?;
    state(out, value)
}
pub fn list<W: ReplyWriter>(
    out: &mut W,
    result: i32,
    values: &[PowerStateSubsystemSleepState],
) -> Result<(), W::Error> {
    if values.len() > MAX_LIST_STATES {
        return Err(out.invalid_value());
    }
    out.ok_status()?;
    out.int32(result)?;
    // The only enabled backend can return one APSS state.
    out.int32(values.len() as i32)?;
    for value in values {
        state(out, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, PartialEq)]
    enum Field {
        Status,
        I32(i32),
        I64(i64),
        Text(String),
        Begin,
        End,
    }
    #[derive(Default)]
    struct Mock {
        fields: Vec<Field>,
        fail_at: Option<usize>,
    }
    impl Mock {
        fn push(&mut self, field: Field) -> Result<(), ()> {
            if self.fail_at == Some(self.fields.len()) {
                return Err(());
            }
            self.fields.push(field);
            Ok(())
        }
    }
    impl ReplyWriter for Mock {
        type Error = ();
        fn invalid_value(&self) {}
        fn ok_status(&mut self) -> Result<(), ()> {
            self.push(Field::Status)
        }
        fn int32(&mut self, v: i32) -> Result<(), ()> {
            self.push(Field::I32(v))
        }
        fn int64(&mut self, v: i64) -> Result<(), ()> {
            self.push(Field::I64(v))
        }
        fn string(&mut self, v: &str) -> Result<(), ()> {
            self.push(Field::Text(v.to_owned()))
        }
        fn begin_state(&mut self) -> Result<i32, ()> {
            self.push(Field::Begin)?;
            Ok(0)
        }
        fn end_state(&mut self, _start: i32) -> Result<(), ()> {
            self.push(Field::End)
        }
    }
    #[test]
    fn donor_result_precedes_out_and_all_six_fields_keep_order() {
        let value = PowerStateSubsystemSleepState {
            name: "APSS".to_owned(),
            version: 1,
            residency_in_msec_since_boot: 254,
            total_transitions: 5,
            last_entry_timestamp_ms: 1000,
            supported_only_in_suspend: true,
        };
        let mut out = Mock::default();
        single(&mut out, 0, &value).unwrap();
        assert_eq!(
            out.fields,
            vec![
                Field::Status,
                Field::I32(0),
                Field::Begin,
                Field::Text("APSS".to_owned()),
                Field::I64(1),
                Field::I64(254),
                Field::I64(5),
                Field::I64(1000),
                Field::I32(1),
                Field::End
            ]
        );
        let mut out = Mock::default();
        list(&mut out, -1, &[]).unwrap();
        assert_eq!(
            out.fields,
            vec![Field::Status, Field::I32(-1), Field::I32(0)]
        );
        let mut out = Mock::default();
        list(&mut out, 0, &[value]).unwrap();
        assert_eq!(
            &out.fields[..4],
            &[Field::Status, Field::I32(0), Field::I32(1), Field::Begin]
        );
    }
    #[test]
    fn oversized_lists_are_rejected_before_any_output() {
        let mut out = Mock::default();
        assert!(list(&mut out, 0, &vec![Default::default(); MAX_LIST_STATES + 1]).is_err());
        assert!(out.fields.is_empty());
    }
    #[test]
    fn write_errors_are_not_suppressed() {
        let mut out = Mock {
            fields: vec![],
            fail_at: Some(1),
        };
        assert!(single(&mut out, -1, &Default::default()).is_err());
        assert_eq!(out.fields, vec![Field::Status]);
    }
    #[test]
    fn size_null_and_trailing_checks_are_bounded() {
        assert_eq!(string_length(-1), Ok(None));
        assert_eq!(string_length(-2), Err(STATUS_BAD_VALUE));
        assert_eq!(string_length(0), Err(STATUS_BAD_VALUE));
        assert_eq!(string_length(4098), Err(STATUS_BAD_VALUE));
        assert_eq!(consumed(4, 8), Err(STATUS_BAD_VALUE));
        assert_eq!(consumed(4, 4), Ok(()));
        assert_eq!(parcelable_size(4, 8), Ok(4));
        assert_eq!(parcelable_size(8, 4), Err(STATUS_BAD_VALUE));
        assert_eq!(parcelable_size(i32::MIN, i32::MAX), Err(STATUS_BAD_VALUE));
        assert_eq!(STATUS_UNKNOWN_TRANSACTION, -74);
    }
}
