use postgres_db::{
    connection::DbConnectionInTransaction,
    custom_types::Semver,
    diff_log::DiffLogInstruction,
    packument::{PackageOnlyPackument, VersionOnlyPackument},
};

pub fn process_entry(
    conn: &mut DbConnectionInTransaction,
    package: String,
    instr: DiffLogInstruction,
) {
    match instr {
        DiffLogInstruction::CreatePackage(data) => create_package(conn, package, data),
        DiffLogInstruction::UpdatePackage(data) => update_package(conn, package, data),
        DiffLogInstruction::PatchPackageReferences => patch_package_refs(conn, package),
        DiffLogInstruction::CreateVersion(v, data) => create_version(conn, package, v, data),
        DiffLogInstruction::UpdateVersion(v, data) => update_version(conn, package, v, data),
        DiffLogInstruction::DeleteVersion(v) => delete_version(conn, package, v),
    }
}

fn create_package(
    conn: &mut DbConnectionInTransaction,
    package: String,
    data: PackageOnlyPackument,
) {
}

fn update_package(
    conn: &mut DbConnectionInTransaction,
    package: String,
    data: PackageOnlyPackument,
) {
}

fn patch_package_refs(conn: &mut DbConnectionInTransaction, package: String) {}

fn create_version(
    conn: &mut DbConnectionInTransaction,
    package: String,
    version: Semver,
    data: VersionOnlyPackument,
) {
}

fn update_version(
    conn: &mut DbConnectionInTransaction,
    package: String,
    version: Semver,
    data: VersionOnlyPackument,
) {
}

fn delete_version(conn: &mut DbConnectionInTransaction, package: String, version: Semver) {}
