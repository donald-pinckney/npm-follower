use super::sql_types::*;
use super::DiffTypeEnum;
use diesel::deserialize;
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output};
use diesel::types::{FromSql, ToSql};
use std::io::Write;

impl ToSql<DiffTypeEnumSql, Pg> for DiffTypeEnum {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            DiffTypeEnum::CreatePackage => out.write_all(b"create_package")?,
            DiffTypeEnum::UpdatePackage => out.write_all(b"update_package")?,
            // DiffTypeEnum::SetPackageLatestTag => out.write_all(b"set_package_latest_tag")?,
            DiffTypeEnum::PatchPackageReferences => out.write_all(b"patch_package_references")?,
            DiffTypeEnum::DeletePackage => out.write_all(b"delete_package")?,
            DiffTypeEnum::CreateVersion => out.write_all(b"create_version")?,
            DiffTypeEnum::UpdateVersion => out.write_all(b"update_version")?,
            DiffTypeEnum::DeleteVersion => out.write_all(b"delete_version")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<DiffTypeEnumSql, Pg> for DiffTypeEnum {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        match not_none!(bytes) {
            b"create_package" => Ok(DiffTypeEnum::CreatePackage),
            b"update_package" => Ok(DiffTypeEnum::UpdatePackage),
            // b"set_package_latest_tag" => Ok(DiffTypeEnum::SetPackageLatestTag),
            b"patch_package_references" => Ok(DiffTypeEnum::PatchPackageReferences),
            b"delete_package" => Ok(DiffTypeEnum::DeletePackage),
            b"create_version" => Ok(DiffTypeEnum::CreateVersion),
            b"update_version" => Ok(DiffTypeEnum::UpdateVersion),
            b"delete_version" => Ok(DiffTypeEnum::DeleteVersion),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}
