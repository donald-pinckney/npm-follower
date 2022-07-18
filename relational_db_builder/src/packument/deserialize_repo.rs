use postgres_db::custom_types::{RepoInfo, Vcs};
use serde_json::Value;
use utils::RemoveInto;
use super::RepositoryInfo;


fn deserialize_repo_infer_type_str(repo: String) -> RepoInfo {
    todo!()
}



fn deserialize_repo_check_git_type_str(repo: String) -> RepoInfo {
    // for now its the same parsing logic, but maybe we handle this differently in the future
    let info = deserialize_repo_infer_type_str(repo);
    assert_eq!(info.vcs, Vcs::Git);
    info
}

pub fn deserialize_repo_blob(repo_blob: Value) -> RepositoryInfo {
    println!("{:?}", repo_blob);

    let info = match repo_blob.clone() {
        Value::String(repo) => deserialize_repo_infer_type_str(repo),
        Value::Object(mut repo_obj) => {
            let t = repo_obj.remove_key_unwrap_type::<String>("type");
            let dir = repo_obj.remove_key_unwrap_type::<String>("directory");
            let url = repo_obj.remove_key_unwrap_type::<String>("url").unwrap();

            let info = match t.as_deref() {
                None => deserialize_repo_infer_type_str(url),
                Some("git" | "github") => deserialize_repo_check_git_type_str(url),
                _ => panic!("Unknown repo type: {:?}", t)
            };

            let parsed_dir = match dir {
                None => info.cloneable_repo_dir,
                Some(json_dir) if info.cloneable_repo_dir == "/" => json_dir,
                Some(json_dir) => {
                    assert_eq!(json_dir, info.cloneable_repo_dir);
                    json_dir
                }
            };

            let final_info = RepoInfo {
                cloneable_repo_dir: parsed_dir,
                ..info
            };

            final_info
        },
        _ => panic!("Can't parse repo: {:?}", repo_blob)
    };

    RepositoryInfo { raw: repo_blob, info }
}


#[cfg(test)]
mod tests {
    use crate::packument::RepositoryInfo;
    use serde_json::{Value, json};
    use test_case::test_case;
    use postgres_db::custom_types::{RepoInfo, Vcs, RepoHostInfo};
    use super::deserialize_repo_blob;


    // github implied shorthand cases
    #[test_case("github/fetch", "https://github.com/github/fetch", "/", "github", "fetch")]
    #[test_case("stuff/stuff.com", "https://github.com/stuff/stuff.com", "/", "github", "fetch")]
    #[test_case("jshttp/accepts", "https://github.com/jshttp/accepts", "/", "github", "fetch")]
    #[test_case("git/git", "https://github.com/git/git", "/", "github", "fetch")]
    #[test_case("git/git.git", "https://github.com/git/git", "/", "github", "fetch")]
    #[test_case("github/github", "https://github.com/github/github", "/", "github", "fetch")]
    #[test_case("github/github.git", "https://github.com/github/github", "/", "github", "fetch")]
    #[test_case("ssh/ssh", "https://github.com/ssh/ssh", "/", "github", "fetch")]
    #[test_case("https/https", "https://github.com/https/https", "/", "github", "fetch")]
    #[test_case("https/github", "https://github.com/https/github", "/", "github", "fetch")]

    // github: shorthand cases
    #[test_case("github:eemeli/yaml", "https://github.com/eemeli/yaml", "/", "github", "fetch")]
    #[test_case("github:stuff.com/stuff.com", "https://github.com/stuff.com/stuff.com", "/", "github", "fetch")]
    #[test_case("github:git/git", "https://github.com/git/git", "/", "github", "fetch")]
    #[test_case("github:git/git.git", "https://github.com/git/git", "/", "github", "fetch")]
    #[test_case("github:github/github", "https://github.com/github/github", "/", "github", "fetch")]
    #[test_case("github:github/github.git", "https://github.com/github/github", "/", "github", "fetch")]
    #[test_case("github:ssh/ssh", "https://github.com/ssh/ssh", "/", "github", "fetch")]
    #[test_case("github:https/https", "https://github.com/https/https", "/", "github", "fetch")]
    #[test_case("github:https/github", "https://github.com/https/github", "/", "github", "fetch")]

    // standard https:// cases
    #[test_case("https://github.com/npm/cacache", "https://github.com/npm/cacache", "/", "github", "fetch")]
    #[test_case("https://github.com/kornelski/https.git-github.git", "https://github.com/kornelski/https.git-github", "/", "github", "fetch")]

    // http:// case
    #[test_case("http://github.com/isaacs/abbrev-js", "https://github.com/isaacs/abbrev-js", "/", "github", "fetch")]

    // broken url case
    #[test_case("https://github.com:crypto-browserify/browserify-rsa.git", "https://github.com/crypto-browserify/browserify-rsa", "/", "github", "fetch")]

    // github tree directory case
    #[test_case("https://github.com/babel/babel/tree/master/packages/babel-plugin-syntax-async-generators", "https://github.com/babel/babel", "packages/babel-plugin-syntax-async-generators", "github", "fetch")]

    // git:// cases
    #[test_case("git://github.com/whitequark/ipaddr.js", "https://github.com/whitequark/ipaddr.js", "/", "github", "fetch")]
    #[test_case("git://github.com/browserify/console-browserify.git", "https://github.com/browserify/console-browserify", "/", "github", "fetch")]

    // git+https:// cases
    #[test_case("git+https://github.com/yargs/set-blocking.git", "https://github.com/yargs/set-blocking", "/", "github", "fetch")]

    // git+ssh:// cases
    #[test_case("git+ssh://git@github.com/mikaelbr/node-notifier.git", "https://github.com/mikaelbr/node-notifier", "/", "github", "fetch")]

    // ssh case
    #[test_case("git@github.com:tsertkov/exec-sh.git", "https://github.com/tsertkov/exec-sh", "/", "github", "fetch")]
    fn test_deserialize_repo_blob_github(url_str: &str, answer_url: &str, answer_dir: &str, answer_user: &str, answer_repo: &str) {
        let blob1: Value = json!(url_str);
        let blob2 = json!({"url": url_str});
        let blob3 = json!({"url": url_str, "type": "git"});
        let blob4 = json!({"url": url_str, "type": "github"});

        let answer_info = RepoInfo { cloneable_repo_url: answer_url.into(), cloneable_repo_dir: answer_dir.into(), vcs: Vcs::Git, host_info: RepoHostInfo::Github { user: answer_user.into(), repo: answer_repo.into() }};
        
        let answer1 = RepositoryInfo { raw: blob1.clone(), info: answer_info.clone() };
        let answer2 = RepositoryInfo { raw: blob2.clone(), info: answer_info.clone() };
        let answer3 = RepositoryInfo { raw: blob3.clone(), info: answer_info.clone() };
        let answer4 = RepositoryInfo { raw: blob4.clone(), info: answer_info };
        
        assert_eq!(deserialize_repo_blob(blob1), answer1);
        assert_eq!(deserialize_repo_blob(blob2), answer2);
        assert_eq!(deserialize_repo_blob(blob3), answer3);
        assert_eq!(deserialize_repo_blob(blob4), answer4);
    }


    // normal bitbucket case
    #[test_case("https://bitbucket.org/janouwehand/stuff-stuff-stuff.git", "https://bitbucket.org/janouwehand/stuff-stuff-stuff", "/", "janouwehand", "stuff-stuff-stuff")]
    #[test_case("http://bitbucket.org/janouwehand/stuff-stuff-stuff.git", "https://bitbucket.org/janouwehand/stuff-stuff-stuff", "/", "janouwehand", "stuff-stuff-stuff")]
    // bitbucket: shorthand
    #[test_case("bitbucket:github/git", "https://bitbucket.org/github/git", "/", "github", "git")]
    // bitbucket tree directory case
    #[test_case("https://bitbucket.org/janouwehand/stuff-stuff-stuff/src/master/ReplacePackageRefs/Properties/", "https://bitbucket.org/janouwehand/stuff-stuff-stuff", "ReplacePackageRefs/Properties/", "janouwehand", "stuff-stuff-stuff")]
    fn test_deserialize_repo_blob_bitbucket(url_str: &str, answer_url: &str, answer_dir: &str, answer_user: &str, answer_repo: &str) {
        let blob1: Value = json!(url_str);
        let blob2 = json!({"url": url_str});
        let blob3 = json!({"url": url_str, "type": "git"});
        let blob4 = json!({"url": url_str, "type": "bitbucket"});

        let answer_info = RepoInfo { cloneable_repo_url: answer_url.into(), cloneable_repo_dir: answer_dir.into(), vcs: Vcs::Git, host_info: RepoHostInfo::Bitbucket { user: answer_user.into(), repo: answer_repo.into() }};
        
        let answer1 = RepositoryInfo { raw: blob1.clone(), info: answer_info.clone() };
        let answer2 = RepositoryInfo { raw: blob2.clone(), info: answer_info.clone() };
        let answer3 = RepositoryInfo { raw: blob3.clone(), info: answer_info.clone() };
        let answer4 = RepositoryInfo { raw: blob4.clone(), info: answer_info };
        
        assert_eq!(deserialize_repo_blob(blob1), answer1);
        assert_eq!(deserialize_repo_blob(blob2), answer2);
        assert_eq!(deserialize_repo_blob(blob3), answer3);
        assert_eq!(deserialize_repo_blob(blob4), answer4);
    }


    // normal gitlab case
    #[test_case("https://gitlab.com/gitlab-org/gitlab.git", "https://gitlab.com/gitlab-org/gitlab.git", "/", "gitlab-org", "gitlab")]
    #[test_case("http://gitlab.com/gitlab-org/gitlab.git", "https://gitlab.com/gitlab-org/gitlab.git", "/", "gitlab-org", "gitlab")]
    // gitlab: shorthand
    #[test_case("gitlab:bitbucket-gist/github", "https://gitlab.com/bitbucket-gist/github.git", "/", "bitbucket-gist", "github")]
    // gitlab tree directory case
    #[test_case("https://gitlab.com/gitlab-org/gitlab/-/tree/master/generator_templates/snowplow_event_definition", "https://gitlab.com/gitlab-org/gitlab.git", "generator_templates/snowplow_event_definition", "gitlab-org", "gitlab")]
    fn test_deserialize_repo_blob_gitlab(url_str: &str, answer_url: &str, answer_dir: &str, answer_user: &str, answer_repo: &str) {
        let blob1: Value = json!(url_str);
        let blob2 = json!({"url": url_str});
        let blob3 = json!({"url": url_str, "type": "git"});
        let blob4 = json!({"url": url_str, "type": "gitlab"});

        let answer_info = RepoInfo { cloneable_repo_url: answer_url.into(), cloneable_repo_dir: answer_dir.into(), vcs: Vcs::Git, host_info: RepoHostInfo::Gitlab { user: answer_user.into(), repo: answer_repo.into() }};
        
        let answer1 = RepositoryInfo { raw: blob1.clone(), info: answer_info.clone() };
        let answer2 = RepositoryInfo { raw: blob2.clone(), info: answer_info.clone() };
        let answer3 = RepositoryInfo { raw: blob3.clone(), info: answer_info.clone() };
        let answer4 = RepositoryInfo { raw: blob4.clone(), info: answer_info };
        
        assert_eq!(deserialize_repo_blob(blob1), answer1);
        assert_eq!(deserialize_repo_blob(blob2), answer2);
        assert_eq!(deserialize_repo_blob(blob3), answer3);
        assert_eq!(deserialize_repo_blob(blob4), answer4);
    }


    // other gist url cases
    #[test_case("https://gist.github.com/mathcube7/35d6483aea16c4f11e9acc51ea659b99", "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99", "35d6483aea16c4f11e9acc51ea659b99")]
    #[test_case("https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99.git", "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99", "35d6483aea16c4f11e9acc51ea659b99")]
    #[test_case("git@gist.github.com:35d6483aea16c4f11e9acc51ea659b99.git", "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99", "35d6483aea16c4f11e9acc51ea659b99")]
    #[test_case("http://gist.github.com/mathcube7/35d6483aea16c4f11e9acc51ea659b99", "https://gist.github.com/35d6483aea16c4f11e9acc51ea659b99", "35d6483aea16c4f11e9acc51ea659b99")]

    // gist: shorthand case
    #[test_case("gist:11081aaa281", "https://gist.github.com/11081aaa281", "11081aaa281")]
    fn test_deserialize_repo_blob_gist(url_str: &str, answer_url: &str, answer_id: &str) {
        let blob1: Value = json!(url_str);
        let blob2 = json!({"url": url_str});
        let blob3 = json!({"url": url_str, "type": "git"});
        let blob4 = json!({"url": url_str, "type": "gist"});

        let answer_info = RepoInfo { cloneable_repo_url: answer_url.into(), cloneable_repo_dir: "/".into(), vcs: Vcs::Git, host_info: RepoHostInfo::Gist { id: answer_id.into() }};
        
        let answer1 = RepositoryInfo { raw: blob1.clone(), info: answer_info.clone() };
        let answer2 = RepositoryInfo { raw: blob2.clone(), info: answer_info.clone() };
        let answer3 = RepositoryInfo { raw: blob3.clone(), info: answer_info.clone() };
        let answer4 = RepositoryInfo { raw: blob4.clone(), info: answer_info };
        
        assert_eq!(deserialize_repo_blob(blob1), answer1);
        assert_eq!(deserialize_repo_blob(blob2), answer2);
        assert_eq!(deserialize_repo_blob(blob3), answer3);
        assert_eq!(deserialize_repo_blob(blob4), answer4);
    }


    // 3rd party git host
    #[test_case("git@git.coolaj86.com:coolaj86/atob.js.git", "git@git.coolaj86.com:coolaj86/atob.js.git")]
    #[test_case("git://git.coolaj86.com/coolaj86/atob.js.git", "https://git.coolaj86.com/coolaj86/atob.js")]
    #[test_case("https://git.coolaj86.com/coolaj86/atob.js.git", "https://git.coolaj86.com/coolaj86/atob.js")]
    #[test_case("http://git.coolaj86.com/coolaj86/atob.js.git", "http://git.coolaj86.com/coolaj86/atob.js")]
    fn test_deserialize_repo_blob_3rd(url_str: &str, answer_url: &str) {
        let blob1: Value = json!(url_str);
        let blob2 = json!({"url": url_str});
        let blob3 = json!({"url": url_str, "type": "git"});

        let answer_info = RepoInfo { cloneable_repo_url: answer_url.into(), cloneable_repo_dir: "/".into(), vcs: Vcs::Git, host_info: RepoHostInfo::Thirdparty};
        
        let answer1 = RepositoryInfo { raw: blob1.clone(), info: answer_info.clone() };
        let answer2 = RepositoryInfo { raw: blob2.clone(), info: answer_info.clone() };
        let answer3 = RepositoryInfo { raw: blob3.clone(), info: answer_info.clone() };
        
        assert_eq!(deserialize_repo_blob(blob1), answer1);
        assert_eq!(deserialize_repo_blob(blob2), answer2);
        assert_eq!(deserialize_repo_blob(blob3), answer3);
    }






    #[test_case("github/fetch", "some/dir")]
    #[test_case("github:git/git.git", "some/dir")]
    #[test_case("https://github.com/kornelski/https.git-github.git", "some/dir")]
    #[test_case("https://github.com/babel/babel/tree/master/packages/babel-plugin-syntax-async-generators", "packages/babel-plugin-syntax-async-generators")]
    #[test_case("git+ssh://git@github.com/istanbuljs/istanbuljs.git", "packages/istanbul-lib-coverage")]
    fn test_deserialize_repo_blob_git_url_with_directory(url_str: &str, directory_str: &str) {
        let blob1 = json!({"url": url_str, "directory": directory_str});
        let blob2 = json!({"url": url_str, "type": "git", "directory": directory_str});
        let blob3 = json!({"url": url_str, "type": "github", "directory": directory_str});

        assert_eq!(deserialize_repo_blob(blob1).info.cloneable_repo_dir, directory_str);
        assert_eq!(deserialize_repo_blob(blob2).info.cloneable_repo_dir, directory_str);
        assert_eq!(deserialize_repo_blob(blob3).info.cloneable_repo_dir, directory_str);
    }
}