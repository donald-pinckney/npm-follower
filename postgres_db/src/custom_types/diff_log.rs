use crate::schema::sql_types::DiffType;

use super::DiffTypeEnum;
use diesel::deserialize;
use diesel::deserialize::FromSql;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, IsNull, Output, ToSql};
use std::io::Write;

impl ToSql<DiffType, Pg> for DiffTypeEnum {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        match self {
            DiffTypeEnum::CreatePackage => out.write_all(b"create_package")?,
            DiffTypeEnum::UpdatePackage => out.write_all(b"update_package")?,
            // DiffTypeEnum::SetPackageLatestTag => out.write_all(b"set_package_latest_tag")?,
            DiffTypeEnum::PatchPackageReferences => out.write_all(b"patch_package_references")?,
            DiffTypeEnum::CreateVersion => out.write_all(b"create_version")?,
            DiffTypeEnum::UpdateVersion => out.write_all(b"update_version")?,
            DiffTypeEnum::DeleteVersion => out.write_all(b"delete_version")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<DiffType, Pg> for DiffTypeEnum {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let bytes = bytes.as_bytes();

        match bytes {
            b"create_package" => Ok(DiffTypeEnum::CreatePackage),
            b"update_package" => Ok(DiffTypeEnum::UpdatePackage),
            // b"set_package_latest_tag" => Ok(DiffTypeEnum::SetPackageLatestTag),
            b"patch_package_references" => Ok(DiffTypeEnum::PatchPackageReferences),
            b"create_version" => Ok(DiffTypeEnum::CreateVersion),
            b"update_version" => Ok(DiffTypeEnum::UpdateVersion),
            b"delete_version" => Ok(DiffTypeEnum::DeleteVersion),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}
