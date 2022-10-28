use std::io::Write;

use crate::schema::sql_types::RepoInfoStruct;

use super::{sql_types::*, RepoHostInfo, RepoInfo, Vcs};
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, IsNull, Output, ToSql, WriteTuple};
use diesel::sql_types::{Nullable, Record, Text};

// ---------- RepoInfo <----> RepoInfoStruct

type RepoInfoStructRecordSql = (
    Text,
    Text,
    VcsEnumSql,
    RepoHostEnumSql,
    Nullable<Text>,
    Nullable<Text>,
    Nullable<Text>,
);

type RepoInfoStructRecordRust = (
    String,
    String,
    Vcs,
    RepoHostEnum,
    Option<String>,
    Option<String>,
    Option<String>,
);

impl ToSql<RepoInfoStruct, Pg> for RepoInfo {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let record: RepoInfoStructRecordRust = match &self.host_info {
            RepoHostInfo::Github { user, repo } => (
                self.cloneable_repo_url.clone(),
                self.cloneable_repo_dir.clone(),
                self.vcs,
                RepoHostEnum::Github,
                Some(user.clone()),
                Some(repo.clone()),
                None,
            ),
            RepoHostInfo::Bitbucket { user, repo } => (
                self.cloneable_repo_url.clone(),
                self.cloneable_repo_dir.clone(),
                self.vcs,
                RepoHostEnum::Bitbucket,
                Some(user.clone()),
                Some(repo.clone()),
                None,
            ),
            RepoHostInfo::Gitlab { user, repo } => (
                self.cloneable_repo_url.clone(),
                self.cloneable_repo_dir.clone(),
                self.vcs,
                RepoHostEnum::Gitlab,
                Some(user.clone()),
                Some(repo.clone()),
                None,
            ),
            RepoHostInfo::Gist { id } => (
                self.cloneable_repo_url.clone(),
                self.cloneable_repo_dir.clone(),
                self.vcs,
                RepoHostEnum::Gist,
                None,
                None,
                Some(id.clone()),
            ),
            RepoHostInfo::Thirdparty => (
                self.cloneable_repo_url.clone(),
                self.cloneable_repo_dir.clone(),
                self.vcs,
                RepoHostEnum::Thirdparty,
                None,
                None,
                None,
            ),
        };

        WriteTuple::<RepoInfoStructRecordSql>::write_tuple(&record, out)
    }
}

impl FromSql<RepoInfoStruct, Pg> for RepoInfo {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let tup: RepoInfoStructRecordRust =
            FromSql::<Record<RepoInfoStructRecordSql>, Pg>::from_sql(bytes)?;
        let (url, dir, vcs) = (tup.0, tup.1, tup.2);
        let host_info_res: deserialize::Result<RepoHostInfo> = match (tup.3, tup.4, tup.5, tup.6) {
            (RepoHostEnum::Github, Some(user), Some(repo), None) => {
                Ok(RepoHostInfo::Github { user, repo })
            }
            (RepoHostEnum::Bitbucket, Some(user), Some(repo), None) => {
                Ok(RepoHostInfo::Bitbucket { user, repo })
            }
            (RepoHostEnum::Gitlab, Some(user), Some(repo), None) => {
                Ok(RepoHostInfo::Gitlab { user, repo })
            }
            (RepoHostEnum::Gist, None, None, Some(id)) => Ok(RepoHostInfo::Gist { id }),
            (RepoHostEnum::Thirdparty, None, None, None) => Ok(RepoHostInfo::Thirdparty),
            _ => Err("Unrecognized enum variant".into()),
        };
        let host_info = host_info_res?;

        Ok(RepoInfo {
            cloneable_repo_url: url,
            cloneable_repo_dir: dir,
            vcs,
            host_info,
        })
    }
}

// ---------- RepoHostEnum <----> RepoHostEnumSql

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "RepoHostEnumSql"]
enum RepoHostEnum {
    Github,
    Bitbucket,
    Gitlab,
    Gist,
    Thirdparty,
}

#[derive(SqlType)]
#[postgres(type_name = "repo_host_enum")]
struct RepoHostEnumSql;

impl ToSql<RepoHostEnumSql, Pg> for RepoHostEnum {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        match self {
            RepoHostEnum::Github => out.write_all(b"github")?,
            RepoHostEnum::Bitbucket => out.write_all(b"bitbucket")?,
            RepoHostEnum::Gitlab => out.write_all(b"gitlab")?,
            RepoHostEnum::Gist => out.write_all(b"gist")?,
            RepoHostEnum::Thirdparty => out.write_all(b"3rdparty")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<RepoHostEnumSql, Pg> for RepoHostEnum {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let bytes = bytes.as_bytes();

        match bytes {
            b"github" => Ok(RepoHostEnum::Github),
            b"bitbucket" => Ok(RepoHostEnum::Bitbucket),
            b"gitlab" => Ok(RepoHostEnum::Gitlab),
            b"gist" => Ok(RepoHostEnum::Gist),
            b"3rdparty" => Ok(RepoHostEnum::Thirdparty),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

// ---------- Vcs <----> VcsEnumSql

#[derive(SqlType)]
#[postgres(type_name = "vcs_type_enum")]
struct VcsEnumSql;

impl ToSql<VcsEnumSql, Pg> for Vcs {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        match self {
            Vcs::Git => out.write_all(b"git")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<VcsEnumSql, Pg> for Vcs {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        let bytes = bytes.as_bytes();

        match bytes {
            b"git" => Ok(Vcs::Git),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

// Unit tests
#[cfg(test)]
mod tests {
    use crate::custom_types::RepoHostInfo;
    use crate::custom_types::RepoInfo;
    use crate::custom_types::Vcs;
    use crate::testing;
    use diesel::prelude::*;
    use diesel::RunQueryDsl;

    table! {
        use diesel::sql_types::*;
        use crate::schema::sql_types::RepoInfoStruct;

        test_repo_info_to_sql {
            id -> Integer,
            r -> Nullable<RepoInfoStruct>,
        }
    }

    #[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
    #[table_name = "test_repo_info_to_sql"]
    struct TestRepoInfoToSql {
        id: i32,
        r: Option<RepoInfo>,
    }

    #[test]
    fn test_repository_type_to_sql_fn() {
        use self::test_repo_info_to_sql::dsl::*;

        let data = vec![
            TestRepoInfoToSql {
                id: 1,
                r: Option::None,
            },
            TestRepoInfoToSql {
                id: 2,
                r: Some(RepoInfo {
                    cloneable_repo_url: "the url".into(),
                    cloneable_repo_dir: "the dir".into(),
                    vcs: Vcs::Git,
                    host_info: RepoHostInfo::Github {
                        user: "the user".into(),
                        repo: "the repo".into(),
                    },
                }),
            },
            TestRepoInfoToSql {
                id: 3,
                r: Some(RepoInfo {
                    cloneable_repo_url: "the url".into(),
                    cloneable_repo_dir: "the dir".into(),
                    vcs: Vcs::Git,
                    host_info: RepoHostInfo::Bitbucket {
                        user: "the user".into(),
                        repo: "the repo".into(),
                    },
                }),
            },
            TestRepoInfoToSql {
                id: 4,
                r: Some(RepoInfo {
                    cloneable_repo_url: "the url".into(),
                    cloneable_repo_dir: "the dir".into(),
                    vcs: Vcs::Git,
                    host_info: RepoHostInfo::Gitlab {
                        user: "the user".into(),
                        repo: "the repo".into(),
                    },
                }),
            },
            TestRepoInfoToSql {
                id: 5,
                r: Some(RepoInfo {
                    cloneable_repo_url: "the url".into(),
                    cloneable_repo_dir: "the dir".into(),
                    vcs: Vcs::Git,
                    host_info: RepoHostInfo::Gist {
                        id: "the id".into(),
                    },
                }),
            },
            TestRepoInfoToSql {
                id: 6,
                r: Some(RepoInfo {
                    cloneable_repo_url: "the url".into(),
                    cloneable_repo_dir: "the dir".into(),
                    vcs: Vcs::Git,
                    host_info: RepoHostInfo::Thirdparty,
                }),
            },
        ];

        testing::using_test_db(|conn| {
            let _temp_table = testing::TempTable::new(
                conn,
                "test_repo_info_to_sql",
                "id SERIAL PRIMARY KEY, r repo_info",
            );

            let inserted = conn
                .get_results(diesel::insert_into(test_repo_info_to_sql).values(&data))
                .unwrap();
            assert_eq!(data, inserted);

            let filter_all = conn.load(test_repo_info_to_sql.filter(id.ge(1))).unwrap();
            assert_eq!(data, filter_all);
        });
    }
}
