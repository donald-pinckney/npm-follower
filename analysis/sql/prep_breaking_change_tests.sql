create table metadata_analysis.prep_breaking_change_tests as (
    with ok_tests as (
    select 
        test.*, 
        trans_deps.x as cli_trans_popularity, 
        row_number() over (partition by test.lib_v, test.lib_v2 order by trans_deps.x desc) as rank_cli_trans_popularity,
        cli.name as cli_n,
        (cli_vers.repository_parsed).cloneable_repo_url as cli_repo,
        cli_vers.extra_metadata->>'gitHead' as cli_git_head
        
    from metadata_analysis.update_full_client_set test
    inner join metadata_analysis.all_dep_counts trans_deps
    on test.cli_p = trans_deps.pkg and trans_deps.count_type = 'num_transitive_runtime_rev_deps'
    inner join packages cli
    on cli.id = test.cli_p and cli.current_package_state_type = 'normal'
    inner join versions cli_vers
    on 		cli_vers.id = test.cli_v 
        and cli_vers.current_version_state_type = 'normal' 
        and (not cli_vers.repository_parsed is null) 
        and cli_vers.extra_metadata->>'gitHead' is not null
        and (cli_vers.repository_parsed).host <> '3rdparty' and (cli_vers.repository_parsed).host <> 'gist'
        and (cli_vers.repository_parsed).cloneable_repo_dir = ''
    )

    select 
        t.lib_p, 
        lib.name as lib_n, 
        t.lib_v, 
        lib_vers1.semver as lib_semver, 
        t.lib_v2, 
        lib_vers2.semver as lib_semver2, 
        t.cli_p, 
        t.cli_v, 
        t.cli_n, 
        t.cli_repo, 
        t.cli_git_head, 
        t.cli_trans_popularity, 
        t.rank_cli_trans_popularity, 
        t.auto_update
    from ok_tests t
    join packages lib on lib.id = t.lib_p
    join versions lib_vers1 on lib_vers1.id = t.lib_v
    join versions lib_vers2 on lib_vers2.id = t.lib_v2
    where t.rank_cli_trans_popularity <= 100
)