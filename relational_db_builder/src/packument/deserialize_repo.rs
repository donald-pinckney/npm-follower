use super::RepositoryInfo;
use postgres_db::custom_types::{RepoInfo, Vcs};
use serde_json::Value;
use url::Url;
use utils::RemoveInto;

/// This attempts to parse the common repo shorthand form of: xxx/yyy
fn try_parse_user_repo_shorthand(x: &str) -> Option<(&str, &str)> {
    let components: Vec<_> = x.split('/').collect();
    if components.len() == 2 {
        let left = components[0];
        let right = components[1];
        if left.contains(':') || left.contains('@') || right.contains(':') || right.contains('@') {
            None
        } else {
            Some((left, right))
        }
    } else {
        None
    }
}

fn match_strip_start(x: &mut &str, p: &str) -> bool {
    if let Some(new_x) = x.strip_prefix(p) {
        *x = new_x;
        true
    } else {
        false
    }
}

/// This attempts to parse the form: git@xxx:yyy
fn try_parse_git_ssh_format(x: &str) -> Option<(&str, &str)> {
    let mut x_copy = x;
    if match_strip_start(&mut x_copy, "git@") {
        let components: Vec<_> = x_copy.split(':').collect();
        if components.len() != 2 {
            return None;
        };
        let left = components[0];
        let right = components[1];
        if left.contains(':') || left.contains('@') || right.contains(':') || right.contains('@') {
            return None;
        }
        Some((left, right))
    } else {
        None
    }
}

fn parse_gist_path(gist_path: &str) -> RepoInfo {
    if gist_path.contains('/') {
        let (_user, id) = try_parse_user_repo_shorthand(gist_path).unwrap();
        return RepoInfo::new_gist(strip_dot_git(id).to_owned());
    } else {
        return RepoInfo::new_gist(strip_dot_git(gist_path).to_owned());
    }
}

fn parse_url_or_ssh_case(url_or_ssh: &str) -> Option<RepoInfo> {
    // Lets try to parse git ssh format first.
    if let Some((host, path)) = try_parse_git_ssh_format(url_or_ssh) {
        if host == "github.com" {
            let (user, repo) = try_parse_user_repo_shorthand(path)?;
            return Some(RepoInfo::new_github(
                "".to_string(),
                user.to_owned(),
                strip_dot_git(repo).to_owned(),
            ));
        } else if host == "bitbucket.org" {
            let (user, repo) = try_parse_user_repo_shorthand(path)?;
            return Some(RepoInfo::new_bitbucket(
                "".to_string(),
                user.to_owned(),
                strip_dot_git(repo).to_owned(),
            ));
        } else if host == "gitlab.com" {
            println!("path: {}", path);
            let (user, repo) = try_parse_user_repo_shorthand(path)?;
            return Some(RepoInfo::new_gitlab(
                "".to_string(),
                user.to_owned(),
                strip_dot_git(repo).to_owned(),
            ));
        } else if host == "gist.github.com" {
            return Some(parse_gist_path(path));
        } else {
            return Some(RepoInfo::new_thirdparty(
                url_or_ssh.to_owned(),
                "".to_owned(),
            ));
        }
    }

    // Otherwise, we should have a valid URL to parse.
    let repo_url = match Url::parse(url_or_ssh) {
        Ok(u) => u,
        Err(_) => return None,
    };
    let scheme = repo_url.scheme();
    let host = repo_url.host_str()?;
    let maybe_user = repo_url.username();
    let url_path = repo_url.path().strip_prefix('/')?;
    let url_path = url_path.strip_suffix('/').unwrap_or(url_path);

    if scheme == "git+ssh" && maybe_user != "git" {
        return None;
    }

    if host == "github.com" {
        if let Some((user, repo)) = try_parse_user_repo_shorthand(url_path) {
            return Some(RepoInfo::new_github(
                "".to_string(),
                user.to_owned(),
                strip_dot_git(repo).to_owned(),
            ));
        } else {
            // Else we handle github tree directory case
            // Example url_path = "babel/babel/tree/master/packages/babel-plugin-syntax-async-generators"
            let comps: Vec<_> = url_path.split('/').collect();
            let num_comps = comps.len();
            if num_comps < 4 {
                return None; // bad
            }
            let user = comps[0];
            let repo = comps[1];
            if comps[2] != "tree" {
                return None; // bad
            }
            let _branch = comps[3]; // We ignore the branch
            if num_comps == 4 {
                return Some(RepoInfo::new_github(
                    "".to_string(),
                    user.to_owned(),
                    strip_dot_git(repo).to_owned(),
                ));
            } else {
                let dir_path = comps[4..].join("/");
                return Some(RepoInfo::new_github(
                    dir_path,
                    user.to_owned(),
                    strip_dot_git(repo).to_owned(),
                ));
            }
        }
    } else if host == "bitbucket.org" {
        if let Some((user, repo)) = try_parse_user_repo_shorthand(url_path) {
            return Some(RepoInfo::new_bitbucket(
                "".to_string(),
                user.to_owned(),
                strip_dot_git(repo).to_owned(),
            ));
        } else {
            // Else we handle bitbucket tree directory case
            // Example url_path = "janouwehand/stuff-stuff-stuff/src/master/ReplacePackageRefs/Properties"
            let comps: Vec<_> = url_path.split('/').collect();
            let num_comps = comps.len();
            if num_comps < 4 {
                return None;
            }
            let user = comps[0];
            let repo = comps[1];
            if comps[2] != "src" {
                return None;
            }
            let _branch = comps[3]; // We ignore the branch
            if num_comps == 4 {
                return Some(RepoInfo::new_bitbucket(
                    "".to_string(),
                    user.to_owned(),
                    strip_dot_git(repo).to_owned(),
                ));
            } else {
                let dir_path = comps[4..].join("/");
                return Some(RepoInfo::new_bitbucket(
                    dir_path,
                    user.to_owned(),
                    strip_dot_git(repo).to_owned(),
                ));
            }
        }
    } else if host == "gitlab.com" {
        if let Some((user, repo)) = try_parse_user_repo_shorthand(url_path) {
            return Some(RepoInfo::new_gitlab(
                "".to_string(),
                user.to_owned(),
                strip_dot_git(repo).to_owned(),
            ));
        } else {
            // Else we handle gitlab tree directory case
            // Example url_path = "gitlab-org/gitlab/-/tree/master/generator_templates/snowplow_event_definition"
            let comps: Vec<_> = url_path.split('/').collect();
            let num_comps = comps.len();
            if num_comps < 5 {
                return None; // bad
            }
            let user = comps[0];
            let repo = comps[1];
            if comps[2] != "-" && comps[3] != "tree" {
                return None; // bad
            }
            let _branch = comps[4]; // We ignore the branch
            if num_comps == 5 {
                return Some(RepoInfo::new_gitlab(
                    "".to_string(),
                    user.to_owned(),
                    strip_dot_git(repo).to_owned(),
                ));
            } else {
                let dir_path = comps[5..].join("/");
                return Some(RepoInfo::new_gitlab(
                    dir_path,
                    user.to_owned(),
                    strip_dot_git(repo).to_owned(),
                ));
            }
        }
    } else if host == "gist.github.com" {
        Some(parse_gist_path(url_path))
    } else if scheme == "git" {
        // Convert to https and strip .git
        let mut new_url = repo_url.clone();
        new_url.set_path(strip_dot_git(url_path));
        let https_url_str = new_url.to_string().replacen("git://", "https://", 1);
        Some(RepoInfo::new_thirdparty(https_url_str, "".to_owned()))
    } else {
        // Strip .git
        let mut new_url = repo_url.clone();
        new_url.set_path(strip_dot_git(url_path));
        Some(RepoInfo::new_thirdparty(new_url.to_string(), "".to_owned()))
    }
}

fn strip_dot_git(repo: &str) -> &str {
    repo.strip_suffix(".git").unwrap_or(repo)
}

fn deserialize_repo_infer_type_str(full_repo_string: String) -> Option<RepoInfo> {
    let mut repo_str: &str = &full_repo_string;
    if match_strip_start(&mut repo_str, "github:") {
        println!("{}", repo_str);
        let (user, repo) = try_parse_user_repo_shorthand(repo_str)?;
        return Some(RepoInfo::new_github(
            "".to_string(),
            user.to_owned(),
            strip_dot_git(repo).to_owned(),
        ));
    } else if match_strip_start(&mut repo_str, "bitbucket:") {
        let (user, repo) = try_parse_user_repo_shorthand(repo_str)?;
        return Some(RepoInfo::new_bitbucket(
            "".to_string(),
            user.to_owned(),
            strip_dot_git(repo).to_owned(),
        ));
    } else if match_strip_start(&mut repo_str, "gitlab:") {
        let (user, repo) = try_parse_user_repo_shorthand(repo_str)?;
        return Some(RepoInfo::new_gitlab(
            "".to_string(),
            user.to_owned(),
            strip_dot_git(repo).to_owned(),
        ));
    } else if match_strip_start(&mut repo_str, "gist:") {
        return Some(parse_gist_path(repo_str));
    } else if let Some((user, repo)) = try_parse_user_repo_shorthand(repo_str) {
        return Some(RepoInfo::new_github(
            "".to_string(),
            user.to_owned(),
            strip_dot_git(repo).to_owned(),
        ));
    }

    // In this case, we are dealig with either some URL, or some git ssh format.

    // First, try to deal with the known broken github url cases
    // We deal with it by rewriting into a non-broken url, then continuing as normal
    if match_strip_start(&mut repo_str, "https://github.com:") {
        // https://github.com:crypto-browserify/browserify-rsa.git
        let fixed_url_string = "https://github.com/".to_owned() + repo_str;
        parse_url_or_ssh_case(&fixed_url_string)
    } else if repo_str.split('/').count() == 3 && repo_str.starts_with("github.com/") {
        // github.com/makindotcc/McHttpFrida
        assert!(match_strip_start(&mut repo_str, "github.com/"));
        let new_repo_str = format!("https://github.com/{}", repo_str);
        parse_url_or_ssh_case(&new_repo_str)
    } else {
        parse_url_or_ssh_case(repo_str)
    }
}

fn deserialize_repo_check_git_type_str(repo: String) -> Option<RepoInfo> {
    // for now its the same parsing logic, but maybe we handle this differently in the future
    let info = deserialize_repo_infer_type_str(repo)?;
    assert_eq!(info.vcs, Vcs::Git);
    Some(info)
}

pub fn deserialize_repo_blob(repo_blob: Value) -> Option<RepositoryInfo> {
    fn deserialize_help(mut repo_obj: serde_json::Map<String, Value>) -> Option<RepoInfo> {
        let t = repo_obj.remove_key_unwrap_type::<String>("type");
        let dir = repo_obj.remove_key_unwrap_type::<String>("directory");
        let url = repo_obj.remove_key_unwrap_type::<String>("url")?;

        let info = match t.as_deref() {
            None => deserialize_repo_infer_type_str(url)?,
            Some(s) => match s.to_lowercase().as_str() {
                "git" | "github" | "public" | "bitbucket" | "gitlab" | "gist" => {
                    deserialize_repo_check_git_type_str(url)?
                }
                "hg" | "https" | "http" => {
                    return None; 
                }
                _ => {
                    eprintln!("Unknown repo type: {:?}", t);
                    return None;
                }
            },
        };

        let parsed_dir = match dir {
            None => info.cloneable_repo_dir,
            Some(json_dir) if info.cloneable_repo_dir.is_empty() => json_dir,
            Some(json_dir) => json_dir,
        };

        Some(RepoInfo {
            cloneable_repo_dir: parsed_dir,
            ..info
        })
    }
    let info = match repo_blob.clone() {
        Value::String(repo) => deserialize_repo_infer_type_str(repo)?,
        Value::Array(l) if l.len() == 1 => {
            let repo = &l[0];
            deserialize_help(repo.as_object()?.clone())?
        }
        Value::Object(repo_obj) => deserialize_help(repo_obj)?,
        _ => {
            eprintln!("Can't parse repo: {:?}", repo_blob);
            return None;
        }
    };

    Some(RepositoryInfo {
        raw: repo_blob,
        info,
    })
}

#[cfg(test)]
mod tests {
    use super::{deserialize_repo_blob, deserialize_repo_infer_type_str};
    use crate::packument::RepositoryInfo;
    use postgres_db::custom_types::{RepoHostInfo, RepoInfo, Vcs};
    use serde_json::{json, Value};
    use test_case::test_case;

    // github implied shorthand cases
    #[test_case(
        "github/fetch",
        "https://github.com/github/fetch",
        "",
        "github",
        "fetch"
    )]
    #[test_case(
        "stuff/stuff.com",
        "https://github.com/stuff/stuff.com",
        "",
        "stuff",
        "stuff.com"
    )]
    #[test_case(
        "jshttp/accepts",
        "https://github.com/jshttp/accepts",
        "",
        "jshttp",
        "accepts"
    )]
    #[test_case("git/git", "https://github.com/git/git", "", "git", "git")]
    #[test_case("git/git.git", "https://github.com/git/git", "", "git", "git")]
    #[test_case(
        "github/github",
        "https://github.com/github/github",
        "",
        "github",
        "github"
    )]
    #[test_case(
        "github/github.git",
        "https://github.com/github/github",
        "",
        "github",
        "github"
    )]
    #[test_case("ssh/ssh", "https://github.com/ssh/ssh", "", "ssh", "ssh")]
    #[test_case("https/https", "https://github.com/https/https", "", "https", "https")]
    #[test_case(
        "https/github",
        "https://github.com/https/github",
        "",
        "https",
        "github"
    )]
    // github: shorthand cases
    #[test_case(
        "github:eemeli/yaml",
        "https://github.com/eemeli/yaml",
        "",
        "eemeli",
        "yaml"
    )]
    #[test_case(
        "github:stuff.com/stuff.com",
        "https://github.com/stuff.com/stuff.com",
        "",
        "stuff.com",
        "stuff.com"
    )]
    #[test_case("github:git/git", "https://github.com/git/git", "", "git", "git")]
    #[test_case("github:git/git.git", "https://github.com/git/git", "", "git", "git")]
    #[test_case(
        "github:github/github",
        "https://github.com/github/github",
        "",
        "github",
        "github"
    )]
    #[test_case(
        "github:github/github.git",
        "https://github.com/github/github",
        "",
        "github",
        "github"
    )]
    #[test_case("github:ssh/ssh", "https://github.com/ssh/ssh", "", "ssh", "ssh")]
    #[test_case(
        "github:https/https",
        "https://github.com/https/https",
        "",
        "https",
        "https"
    )]
    #[test_case(
        "github:https/github",
        "https://github.com/https/github",
        "",
        "https",
        "github"
    )]
    // standard https:// cases
    #[test_case(
        "https://github.com/npm/cacache",
        "https://github.com/npm/cacache",
        "",
        "npm",
        "cacache"
    )]
    #[test_case(
        "https://github.com/kornelski/https.git-github.git",
        "https://github.com/kornelski/https.git-github",
        "",
        "kornelski",
        "https.git-github"
    )]
    // http:// case
    #[test_case(
        "http://github.com/isaacs/abbrev-js",
        "https://github.com/isaacs/abbrev-js",
        "",
        "isaacs",
        "abbrev-js"
    )]
    // broken url case
    #[test_case(
        "https://github.com:crypto-browserify/browserify-rsa.git",
        "https://github.com/crypto-browserify/browserify-rsa",
        "",
        "crypto-browserify",
        "browserify-rsa"
    )]
    #[test_case(
        "github.com/makindotcc/McHttpFrida",
        "https://github.com/makindotcc/McHttpFrida",
        "",
        "makindotcc",
        "McHttpFrida"
    )]
    // github tree directory case
    #[test_case(
        "https://github.com/babel/babel/tree/master/packages/babel-plugin-syntax-async-generators",
        "https://github.com/babel/babel",
        "packages/babel-plugin-syntax-async-generators",
        "babel",
        "babel"
    )]
    // git:// cases
    #[test_case(
        "git://github.com/whitequark/ipaddr.js",
        "https://github.com/whitequark/ipaddr.js",
        "",
        "whitequark",
        "ipaddr.js"
    )]
    #[test_case(
        "git://github.com/browserify/console-browserify.git",
        "https://github.com/browserify/console-browserify",
        "",
        "browserify",
        "console-browserify"
    )]
    // git+https:// cases
    #[test_case(
        "git+https://github.com/yargs/set-blocking.git",
        "https://github.com/yargs/set-blocking",
        "",
        "yargs",
        "set-blocking"
    )]
    // git+ssh:// cases
    #[test_case(
        "git+ssh://git@github.com/mikaelbr/node-notifier.git",
        "https://github.com/mikaelbr/node-notifier",
        "",
        "mikaelbr",
        "node-notifier"
    )]
    #[test_case(
        "git+ssh://git@github.com/istanbuljs/istanbuljs.git",
        "https://github.com/istanbuljs/istanbuljs",
        "",
        "istanbuljs",
        "istanbuljs"
    )]
    // ssh case
    #[test_case(
        "git@github.com:tsertkov/exec-sh.git",
        "https://github.com/tsertkov/exec-sh",
        "",
        "tsertkov",
        "exec-sh"
    )]
    fn test_deserialize_repo_blob_github(
        url_str: &str,
        answer_url: &str,
        answer_dir: &str,
        answer_user: &str,
        answer_repo: &str,
    ) {
        let blob1: Value = json!(url_str);
        let blob2 = json!({ "url": url_str });
        let blob3 = json!({"url": url_str, "type": "git"});
        let blob4 = json!({"url": url_str, "type": "github"});

        let answer_info = RepoInfo {
            cloneable_repo_url: answer_url.into(),
            cloneable_repo_dir: answer_dir.into(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Github {
                user: answer_user.into(),
                repo: answer_repo.into(),
            },
        };

        let answer1 = RepositoryInfo {
            raw: blob1.clone(),
            info: answer_info.clone(),
        };
        let answer2 = RepositoryInfo {
            raw: blob2.clone(),
            info: answer_info.clone(),
        };
        let answer3 = RepositoryInfo {
            raw: blob3.clone(),
            info: answer_info.clone(),
        };
        let answer4 = RepositoryInfo {
            raw: blob4.clone(),
            info: answer_info,
        };

        assert_eq!(deserialize_repo_blob(blob1).unwrap(), answer1);
        assert_eq!(deserialize_repo_blob(blob2).unwrap(), answer2);
        assert_eq!(deserialize_repo_blob(blob3).unwrap(), answer3);
        assert_eq!(deserialize_repo_blob(blob4).unwrap(), answer4);
    }

    // normal bitbucket case
    #[test_case(
        "https://bitbucket.org/janouwehand/stuff-stuff-stuff.git",
        "https://bitbucket.org/janouwehand/stuff-stuff-stuff",
        "",
        "janouwehand",
        "stuff-stuff-stuff"
    )]
    #[test_case(
        "http://bitbucket.org/janouwehand/stuff-stuff-stuff.git",
        "https://bitbucket.org/janouwehand/stuff-stuff-stuff",
        "",
        "janouwehand",
        "stuff-stuff-stuff"
    )]
    // bitbucket: shorthand
    #[test_case(
        "bitbucket:github/git",
        "https://bitbucket.org/github/git",
        "",
        "github",
        "git"
    )]
    // bitbucket tree directory case
    #[test_case("https://bitbucket.org/janouwehand/stuff-stuff-stuff/src/master/ReplacePackageRefs/Properties/", "https://bitbucket.org/janouwehand/stuff-stuff-stuff", "ReplacePackageRefs/Properties", "janouwehand", "stuff-stuff-stuff")]
    fn test_deserialize_repo_blob_bitbucket(
        url_str: &str,
        answer_url: &str,
        answer_dir: &str,
        answer_user: &str,
        answer_repo: &str,
    ) {
        let blob1: Value = json!(url_str);
        let blob2 = json!({ "url": url_str });
        let blob3 = json!({"url": url_str, "type": "git"});
        let blob4 = json!({"url": url_str, "type": "bitbucket"});

        let answer_info = RepoInfo {
            cloneable_repo_url: answer_url.into(),
            cloneable_repo_dir: answer_dir.into(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Bitbucket {
                user: answer_user.into(),
                repo: answer_repo.into(),
            },
        };

        let answer1 = RepositoryInfo {
            raw: blob1.clone(),
            info: answer_info.clone(),
        };
        let answer2 = RepositoryInfo {
            raw: blob2.clone(),
            info: answer_info.clone(),
        };
        let answer3 = RepositoryInfo {
            raw: blob3.clone(),
            info: answer_info.clone(),
        };
        let answer4 = RepositoryInfo {
            raw: blob4.clone(),
            info: answer_info,
        };

        assert_eq!(deserialize_repo_blob(blob1).unwrap(), answer1);
        assert_eq!(deserialize_repo_blob(blob2).unwrap(), answer2);
        assert_eq!(deserialize_repo_blob(blob3).unwrap(), answer3);
        assert_eq!(deserialize_repo_blob(blob4).unwrap(), answer4);
    }

    // normal gitlab case
    #[test_case(
        "https://gitlab.com/gitlab-org/gitlab.git",
        "https://gitlab.com/gitlab-org/gitlab.git",
        "",
        "gitlab-org",
        "gitlab"
    )]
    #[test_case(
        "http://gitlab.com/gitlab-org/gitlab.git",
        "https://gitlab.com/gitlab-org/gitlab.git",
        "",
        "gitlab-org",
        "gitlab"
    )]
    // gitlab: shorthand
    #[test_case(
        "gitlab:bitbucket-gist/github",
        "https://gitlab.com/bitbucket-gist/github.git",
        "",
        "bitbucket-gist",
        "github"
    )]
    // gitlab tree directory case
    #[test_case("https://gitlab.com/gitlab-org/gitlab/-/tree/master/generator_templates/snowplow_event_definition", "https://gitlab.com/gitlab-org/gitlab.git", "generator_templates/snowplow_event_definition", "gitlab-org", "gitlab")]
    fn test_deserialize_repo_blob_gitlab(
        url_str: &str,
        answer_url: &str,
        answer_dir: &str,
        answer_user: &str,
        answer_repo: &str,
    ) {
        let blob1: Value = json!(url_str);
        let blob2 = json!({ "url": url_str });
        let blob3 = json!({"url": url_str, "type": "git"});
        let blob4 = json!({"url": url_str, "type": "gitlab"});

        let answer_info = RepoInfo {
            cloneable_repo_url: answer_url.into(),
            cloneable_repo_dir: answer_dir.into(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Gitlab {
                user: answer_user.into(),
                repo: answer_repo.into(),
            },
        };

        let answer1 = RepositoryInfo {
            raw: blob1.clone(),
            info: answer_info.clone(),
        };
        let answer2 = RepositoryInfo {
            raw: blob2.clone(),
            info: answer_info.clone(),
        };
        let answer3 = RepositoryInfo {
            raw: blob3.clone(),
            info: answer_info.clone(),
        };
        let answer4 = RepositoryInfo {
            raw: blob4.clone(),
            info: answer_info,
        };

        assert_eq!(deserialize_repo_blob(blob1).unwrap(), answer1);
        assert_eq!(deserialize_repo_blob(blob2).unwrap(), answer2);
        assert_eq!(deserialize_repo_blob(blob3).unwrap(), answer3);
        assert_eq!(deserialize_repo_blob(blob4).unwrap(), answer4);
    }

    // other gist url cases
    #[test_case(
        "https://gist.github.com/mathcube7/35d6483aea16c4f11e9acc51ea659b99",
        "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99",
        "35d6483aea16c4f11e9acc51ea659b99"
    )]
    #[test_case(
        "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99.git",
        "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99",
        "35d6483aea16c4f11e9acc51ea659b99"
    )]
    #[test_case(
        "git@gist.github.com:35d6483aea16c4f11e9acc51ea659b99.git",
        "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99",
        "35d6483aea16c4f11e9acc51ea659b99"
    )]
    #[test_case(
        "http://gist.github.com/mathcube7/35d6483aea16c4f11e9acc51ea659b99",
        "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99",
        "35d6483aea16c4f11e9acc51ea659b99"
    )]
    // gist: shorthand case
    #[test_case(
        "gist:11081aaa281",
        "https://gist.github.com/11081aaa281",
        "11081aaa281"
    )]
    fn test_deserialize_repo_blob_gist(url_str: &str, answer_url: &str, answer_id: &str) {
        let blob1: Value = json!(url_str);
        let blob2 = json!({ "url": url_str });
        let blob3 = json!({"url": url_str, "type": "git"});
        let blob4 = json!({"url": url_str, "type": "gist"});

        let answer_info = RepoInfo {
            cloneable_repo_url: answer_url.into(),
            cloneable_repo_dir: "".into(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Gist {
                id: answer_id.into(),
            },
        };

        let answer1 = RepositoryInfo {
            raw: blob1.clone(),
            info: answer_info.clone(),
        };
        let answer2 = RepositoryInfo {
            raw: blob2.clone(),
            info: answer_info.clone(),
        };
        let answer3 = RepositoryInfo {
            raw: blob3.clone(),
            info: answer_info.clone(),
        };
        let answer4 = RepositoryInfo {
            raw: blob4.clone(),
            info: answer_info,
        };

        assert_eq!(deserialize_repo_blob(blob1).unwrap(), answer1);
        assert_eq!(deserialize_repo_blob(blob2).unwrap(), answer2);
        assert_eq!(deserialize_repo_blob(blob3).unwrap(), answer3);
        assert_eq!(deserialize_repo_blob(blob4).unwrap(), answer4);
    }

    // 3rd party git host
    #[test_case(
        "git@git.coolaj86.com:coolaj86/atob.js.git",
        "git@git.coolaj86.com:coolaj86/atob.js.git"
    )]
    #[test_case(
        "git://git.coolaj86.com/coolaj86/atob.js.git",
        "https://git.coolaj86.com/coolaj86/atob.js"
    )]
    #[test_case(
        "https://git.coolaj86.com/coolaj86/atob.js.git",
        "https://git.coolaj86.com/coolaj86/atob.js"
    )]
    #[test_case(
        "http://git.coolaj86.com/coolaj86/atob.js.git",
        "http://git.coolaj86.com/coolaj86/atob.js"
    )]
    fn test_deserialize_repo_blob_3rd(url_str: &str, answer_url: &str) {
        let blob1: Value = json!(url_str);
        let blob2 = json!({ "url": url_str });
        let blob3 = json!({"url": url_str, "type": "git"});

        let answer_info = RepoInfo {
            cloneable_repo_url: answer_url.into(),
            cloneable_repo_dir: "".into(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Thirdparty,
        };

        let answer1 = RepositoryInfo {
            raw: blob1.clone(),
            info: answer_info.clone(),
        };
        let answer2 = RepositoryInfo {
            raw: blob2.clone(),
            info: answer_info.clone(),
        };
        let answer3 = RepositoryInfo {
            raw: blob3.clone(),
            info: answer_info,
        };

        assert_eq!(deserialize_repo_blob(blob1).unwrap(), answer1);
        assert_eq!(deserialize_repo_blob(blob2).unwrap(), answer2);
        assert_eq!(deserialize_repo_blob(blob3).unwrap(), answer3);
    }

    // github implied shorthand cases
    #[test_case(
        "github/fetch",
        "https://github.com/github/fetch",
        "",
        "github",
        "fetch"
    )]
    #[test_case(
        "stuff/stuff.com",
        "https://github.com/stuff/stuff.com",
        "",
        "stuff",
        "stuff.com"
    )]
    #[test_case(
        "jshttp/accepts",
        "https://github.com/jshttp/accepts",
        "",
        "jshttp",
        "accepts"
    )]
    #[test_case("git/git", "https://github.com/git/git", "", "git", "git")]
    #[test_case("git/git.git", "https://github.com/git/git", "", "git", "git")]
    #[test_case(
        "github/github",
        "https://github.com/github/github",
        "",
        "github",
        "github"
    )]
    #[test_case(
        "github/github.git",
        "https://github.com/github/github",
        "",
        "github",
        "github"
    )]
    #[test_case("ssh/ssh", "https://github.com/ssh/ssh", "", "ssh", "ssh")]
    #[test_case("https/https", "https://github.com/https/https", "", "https", "https")]
    #[test_case(
        "https/github",
        "https://github.com/https/github",
        "",
        "https",
        "github"
    )]
    // github: shorthand cases
    #[test_case(
        "github:eemeli/yaml",
        "https://github.com/eemeli/yaml",
        "",
        "eemeli",
        "yaml"
    )]
    #[test_case(
        "github:stuff.com/stuff.com",
        "https://github.com/stuff.com/stuff.com",
        "",
        "stuff.com",
        "stuff.com"
    )]
    #[test_case("github:git/git", "https://github.com/git/git", "", "git", "git")]
    #[test_case("github:git/git.git", "https://github.com/git/git", "", "git", "git")]
    #[test_case(
        "github:github/github",
        "https://github.com/github/github",
        "",
        "github",
        "github"
    )]
    #[test_case(
        "github:github/github.git",
        "https://github.com/github/github",
        "",
        "github",
        "github"
    )]
    #[test_case("github:ssh/ssh", "https://github.com/ssh/ssh", "", "ssh", "ssh")]
    #[test_case(
        "github:https/https",
        "https://github.com/https/https",
        "",
        "https",
        "https"
    )]
    #[test_case(
        "github:https/github",
        "https://github.com/https/github",
        "",
        "https",
        "github"
    )]
    // standard https:// cases
    #[test_case(
        "https://github.com/npm/cacache",
        "https://github.com/npm/cacache",
        "",
        "npm",
        "cacache"
    )]
    #[test_case(
        "https://github.com/kornelski/https.git-github.git",
        "https://github.com/kornelski/https.git-github",
        "",
        "kornelski",
        "https.git-github"
    )]
    // http:// case
    #[test_case(
        "http://github.com/isaacs/abbrev-js",
        "https://github.com/isaacs/abbrev-js",
        "",
        "isaacs",
        "abbrev-js"
    )]
    // broken url case
    #[test_case(
        "https://github.com:crypto-browserify/browserify-rsa.git",
        "https://github.com/crypto-browserify/browserify-rsa",
        "",
        "crypto-browserify",
        "browserify-rsa"
    )]
    #[test_case(
        "github.com/makindotcc/McHttpFrida",
        "https://github.com/makindotcc/McHttpFrida",
        "",
        "makindotcc",
        "McHttpFrida"
    )]
    // github tree directory case
    #[test_case(
        "https://github.com/babel/babel/tree/master/packages/babel-plugin-syntax-async-generators",
        "https://github.com/babel/babel",
        "packages/babel-plugin-syntax-async-generators",
        "babel",
        "babel"
    )]
    // git:// cases
    #[test_case(
        "git://github.com/whitequark/ipaddr.js",
        "https://github.com/whitequark/ipaddr.js",
        "",
        "whitequark",
        "ipaddr.js"
    )]
    #[test_case(
        "git://github.com/browserify/console-browserify.git",
        "https://github.com/browserify/console-browserify",
        "",
        "browserify",
        "console-browserify"
    )]
    // git+https:// cases
    #[test_case(
        "git+https://github.com/yargs/set-blocking.git",
        "https://github.com/yargs/set-blocking",
        "",
        "yargs",
        "set-blocking"
    )]
    // git+ssh:// cases
    #[test_case(
        "git+ssh://git@github.com/mikaelbr/node-notifier.git",
        "https://github.com/mikaelbr/node-notifier",
        "",
        "mikaelbr",
        "node-notifier"
    )]
    #[test_case(
        "git+ssh://git@github.com/istanbuljs/istanbuljs.git",
        "https://github.com/istanbuljs/istanbuljs",
        "",
        "istanbuljs",
        "istanbuljs"
    )]
    // ssh case
    #[test_case(
        "git@github.com:tsertkov/exec-sh.git",
        "https://github.com/tsertkov/exec-sh",
        "",
        "tsertkov",
        "exec-sh"
    )]
    fn test_deserialize_repo_infer_type_str_github(
        url_str: &str,
        answer_url: &str,
        answer_dir: &str,
        answer_user: &str,
        answer_repo: &str,
    ) {
        let answer_info = RepoInfo {
            cloneable_repo_url: answer_url.into(),
            cloneable_repo_dir: answer_dir.into(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Github {
                user: answer_user.into(),
                repo: answer_repo.into(),
            },
        };
        assert_eq!(
            deserialize_repo_infer_type_str(url_str.to_string()).unwrap(),
            answer_info
        );
    }

    // normal bitbucket case
    #[test_case(
        "https://bitbucket.org/janouwehand/stuff-stuff-stuff.git",
        "https://bitbucket.org/janouwehand/stuff-stuff-stuff",
        "",
        "janouwehand",
        "stuff-stuff-stuff"
    )]
    #[test_case(
        "http://bitbucket.org/janouwehand/stuff-stuff-stuff.git",
        "https://bitbucket.org/janouwehand/stuff-stuff-stuff",
        "",
        "janouwehand",
        "stuff-stuff-stuff"
    )]
    // bitbucket: shorthand
    #[test_case(
        "bitbucket:github/git",
        "https://bitbucket.org/github/git",
        "",
        "github",
        "git"
    )]
    // bitbucket tree directory case
    #[test_case("https://bitbucket.org/janouwehand/stuff-stuff-stuff/src/master/ReplacePackageRefs/Properties/", "https://bitbucket.org/janouwehand/stuff-stuff-stuff", "ReplacePackageRefs/Properties", "janouwehand", "stuff-stuff-stuff")]
    fn test_deserialize_repo_infer_type_str_bitbucket(
        url_str: &str,
        answer_url: &str,
        answer_dir: &str,
        answer_user: &str,
        answer_repo: &str,
    ) {
        let answer_info = RepoInfo {
            cloneable_repo_url: answer_url.into(),
            cloneable_repo_dir: answer_dir.into(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Bitbucket {
                user: answer_user.into(),
                repo: answer_repo.into(),
            },
        };
        assert_eq!(
            deserialize_repo_infer_type_str(url_str.to_string()).unwrap(),
            answer_info
        );
    }

    // normal gitlab case
    #[test_case(
        "https://gitlab.com/gitlab-org/gitlab.git",
        "https://gitlab.com/gitlab-org/gitlab.git",
        "",
        "gitlab-org",
        "gitlab"
    )]
    #[test_case(
        "http://gitlab.com/gitlab-org/gitlab.git",
        "https://gitlab.com/gitlab-org/gitlab.git",
        "",
        "gitlab-org",
        "gitlab"
    )]
    // gitlab: shorthand
    #[test_case(
        "gitlab:bitbucket-gist/github",
        "https://gitlab.com/bitbucket-gist/github.git",
        "",
        "bitbucket-gist",
        "github"
    )]
    // gitlab tree directory case
    #[test_case("https://gitlab.com/gitlab-org/gitlab/-/tree/master/generator_templates/snowplow_event_definition", "https://gitlab.com/gitlab-org/gitlab.git", "generator_templates/snowplow_event_definition", "gitlab-org", "gitlab")]
    fn test_deserialize_repo_infer_type_str_gitlab(
        url_str: &str,
        answer_url: &str,
        answer_dir: &str,
        answer_user: &str,
        answer_repo: &str,
    ) {
        let answer_info = RepoInfo {
            cloneable_repo_url: answer_url.into(),
            cloneable_repo_dir: answer_dir.into(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Gitlab {
                user: answer_user.into(),
                repo: answer_repo.into(),
            },
        };
        assert_eq!(
            deserialize_repo_infer_type_str(url_str.to_string()).unwrap(),
            answer_info
        );
    }

    // other gist url cases
    #[test_case(
        "https://gist.github.com/mathcube7/35d6483aea16c4f11e9acc51ea659b99",
        "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99",
        "35d6483aea16c4f11e9acc51ea659b99"
    )]
    #[test_case(
        "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99.git",
        "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99",
        "35d6483aea16c4f11e9acc51ea659b99"
    )]
    #[test_case(
        "git@gist.github.com:35d6483aea16c4f11e9acc51ea659b99.git",
        "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99",
        "35d6483aea16c4f11e9acc51ea659b99"
    )]
    #[test_case(
        "http://gist.github.com/mathcube7/35d6483aea16c4f11e9acc51ea659b99",
        "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99",
        "35d6483aea16c4f11e9acc51ea659b99"
    )]
    // gist: shorthand case
    #[test_case(
        "gist:11081aaa281",
        "https://gist.github.com/11081aaa281",
        "11081aaa281"
    )]
    fn test_deserialize_repo_infer_type_str_gist(url_str: &str, answer_url: &str, answer_id: &str) {
        let answer_info = RepoInfo {
            cloneable_repo_url: answer_url.into(),
            cloneable_repo_dir: "".into(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Gist {
                id: answer_id.into(),
            },
        };
        assert_eq!(
            deserialize_repo_infer_type_str(url_str.to_string()).unwrap(),
            answer_info
        );
    }

    // 3rd party git host
    #[test_case(
        "git@git.coolaj86.com:coolaj86/atob.js.git",
        "git@git.coolaj86.com:coolaj86/atob.js.git"
    )]
    #[test_case(
        "git://git.coolaj86.com/coolaj86/atob.js.git",
        "https://git.coolaj86.com/coolaj86/atob.js"
    )]
    #[test_case(
        "https://git.coolaj86.com/coolaj86/atob.js.git",
        "https://git.coolaj86.com/coolaj86/atob.js"
    )]
    #[test_case(
        "http://git.coolaj86.com/coolaj86/atob.js.git",
        "http://git.coolaj86.com/coolaj86/atob.js"
    )]
    fn test_deserialize_repo_infer_type_str_3rd(url_str: &str, answer_url: &str) {
        let answer_info = RepoInfo {
            cloneable_repo_url: answer_url.into(),
            cloneable_repo_dir: "".into(),
            vcs: Vcs::Git,
            host_info: RepoHostInfo::Thirdparty,
        };
        assert_eq!(
            deserialize_repo_infer_type_str(url_str.to_string()).unwrap(),
            answer_info
        );
    }
}
