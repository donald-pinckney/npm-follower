use utils::check_no_concurrent_processes;

fn main() {
    check_no_concurrent_processes("relational_db_builder");

    let _conn = postgres_db::connect();

    
}
