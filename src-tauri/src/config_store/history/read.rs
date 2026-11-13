use super::{validate_write_record, ConfigWriteRecord, ProfileStoreError, StoreOperationError};
use asb_core::contracts::AppKind;
use serde::de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

struct History<'a> {
    app: AppKind,
    visit: &'a mut dyn FnMut(ConfigWriteRecord) -> Result<(), String>,
    failure: &'a mut Option<String>,
    invalid: &'a mut Option<String>,
}

impl<'de> DeserializeSeed<'de> for History<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for History<'_> {
    type Value = ();
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("the current configuration history object")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        let mut seen = false;
        while let Some(key) = map.next_key::<String>()? {
            if key != "records" || seen { return Err(M::Error::custom("invalid history field")); }
            map.next_value_seed(Records { app: self.app, visit: &mut *self.visit, failure: &mut *self.failure, invalid: &mut *self.invalid })?;
            seen = true;
        }
        if !seen { return Err(M::Error::missing_field("records")); }
        Ok(())
    }
}

struct Records<'a> {
    app: AppKind,
    visit: &'a mut dyn FnMut(ConfigWriteRecord) -> Result<(), String>,
    failure: &'a mut Option<String>,
    invalid: &'a mut Option<String>,
}

impl<'de> DeserializeSeed<'de> for Records<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Records<'_> {
    type Value = ();
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("configuration write records")
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut sequence: S) -> Result<(), S::Error> {
        while let Some(record) = sequence.next_element::<ConfigWriteRecord>()? {
            if let Err(error) = validate_write_record(self.app, &record) {
                *self.invalid = Some(error.clone());
                return Err(S::Error::custom(error));
            }
            if let Err(error) = (self.visit)(record) {
                *self.failure = Some(error.clone());
                return Err(S::Error::custom(error));
            }
        }
        Ok(())
    }
}

pub(super) fn visit(path: &Path, app: AppKind,
    visit: &mut dyn FnMut(ConfigWriteRecord) -> Result<(), String>) -> Result<(), StoreOperationError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(ProfileStoreError::unreadable(path, &error).into()),
    };
    let mut failure = None;
    let mut invalid = None;
    let mut reader = serde_json::Deserializer::from_reader(BufReader::new(file));
    let result = History { app, visit, failure: &mut failure, invalid: &mut invalid }.deserialize(&mut reader).and_then(|()| reader.end());
    match result {
        Ok(()) => Ok(()),
        Err(_) if invalid.is_some() => Err(ProfileStoreError::invalid(path, invalid.unwrap()).into()),
        Err(_) if failure.is_some() => Err(failure.unwrap().into()),
        Err(error) => Err(super::super::error::json_error(path, &error).into()),
    }
}
